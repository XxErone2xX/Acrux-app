//! Temporary UTF-8 clipboard ownership for the foreground game's native paste.
//! Keep serving the previous text afterwards, including when no macro is active.
use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};
use x11rb::{
    connection::Connection as _,
    protocol::{
        xproto::{self, ConnectionExt as _, EventMask, PropMode, SelectionNotifyEvent},
        Event,
    },
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
    COPY_DEPTH_FROM_PARENT, CURRENT_TIME, NONE,
};

const LIMIT: usize = 64 * 1024;
const DEADLINE: Duration = Duration::from_secs(2);

pub(super) struct Lease {
    restore: Option<Sender<Sender<Result<(), String>>>>,
}
impl Lease {
    pub fn restore(mut self) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.restore
            .take()
            .ok_or("클립보드 연결이 끝났습니다.")?
            .send(tx)
            .map_err(|_| "클립보드를 복원하지 못했습니다.")?;
        rx.recv_timeout(DEADLINE)
            .map_err(|_| "게임의 붙여넣기 응답을 기다리지 못했습니다.")?
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Some(restore) = self.restore.take() {
            let (tx, _) = mpsc::channel();
            let _ = restore.send(tx);
        }
    }
}

pub(super) fn stage(text: String) -> Result<Lease, String> {
    if text.len() > LIMIT || text.encode_utf16().count() > 8192 || text.contains('\0') {
        return Err("입력할 문장이 너무 길거나 유효하지 않습니다.".into());
    }
    let (ready_tx, ready_rx) = mpsc::channel();
    let (restore_tx, restore_rx) = mpsc::channel();
    thread::Builder::new()
        .name("acrux-paste".into())
        .spawn(move || {
            let result = serve(text.into_bytes(), &ready_tx, restore_rx);
            if let Err(error) = result {
                let _ = ready_tx.send(Err(error));
            }
        })
        .map_err(|error| format!("붙여넣기를 준비하지 못했습니다: {error}"))?;
    ready_rx
        .recv_timeout(DEADLINE + Duration::from_millis(100))
        .map_err(|_| "클립보드를 준비하는 시간이 초과됐습니다.")??;
    Ok(Lease {
        restore: Some(restore_tx),
    })
}

struct Selection {
    conn: RustConnection,
    window: u32,
    clipboard: u32,
    utf8: u32,
    targets: u32,
    text: u32,
    plain: u32,
    property: u32,
}
impl Selection {
    fn new() -> Result<Self, String> {
        let (conn, screen) = RustConnection::connect(None).map_err(|e| e.to_string())?;
        let root = conn.setup().roots[screen].root;
        let atom = |name: &[u8]| -> Result<u32, String> {
            Ok(conn
                .intern_atom(false, name)
                .map_err(|e| e.to_string())?
                .reply()
                .map_err(|e| e.to_string())?
                .atom)
        };
        let clipboard = atom(b"CLIPBOARD")?;
        let utf8 = atom(b"UTF8_STRING")?;
        let targets = atom(b"TARGETS")?;
        let text = atom(b"TEXT")?;
        let plain = atom(b"text/plain;charset=utf-8")?;
        let property = atom(b"ACRUX_CLIPBOARD")?;
        let window = conn.generate_id().map_err(|e| e.to_string())?;
        conn.create_window(
            COPY_DEPTH_FROM_PARENT,
            window,
            root,
            0,
            0,
            1,
            1,
            0,
            xproto::WindowClass::INPUT_OUTPUT,
            0,
            &xproto::CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )
        .map_err(|e| e.to_string())?
        .check()
        .map_err(|e| e.to_string())?;
        Ok(Self {
            conn,
            window,
            clipboard,
            utf8,
            targets,
            text,
            plain,
            property,
        })
    }
    fn previous(&self) -> Result<Option<Vec<u8>>, String> {
        if self
            .conn
            .get_selection_owner(self.clipboard)
            .map_err(|e| e.to_string())?
            .reply()
            .map_err(|e| e.to_string())?
            .owner
            == NONE
        {
            return Ok(None);
        }
        let start = Instant::now();
        for target in [self.utf8, xproto::AtomEnum::STRING.into()] {
            self.conn
                .convert_selection(
                    self.window,
                    self.clipboard,
                    target,
                    self.property,
                    CURRENT_TIME,
                )
                .map_err(|e| e.to_string())?;
            self.conn.flush().map_err(|e| e.to_string())?;
            while start.elapsed() < Duration::from_millis(1200) {
                match self.conn.poll_for_event().map_err(|e| e.to_string())? {
                    Some(Event::SelectionNotify(event)) if event.requestor == self.window => {
                        if event.property == NONE {
                            break;
                        }
                        let data = self
                            .conn
                            .get_property(
                                true,
                                self.window,
                                self.property,
                                xproto::AtomEnum::ANY,
                                0,
                                (LIMIT / 4 + 1) as u32,
                            )
                            .map_err(|e| e.to_string())?
                            .reply()
                            .map_err(|e| e.to_string())?;
                        if data.format != 8 || data.bytes_after != 0 || data.value.len() > LIMIT {
                            return Ok(None);
                        }
                        return if target == self.utf8 {
                            Ok(String::from_utf8(data.value).ok().map(String::into_bytes))
                        } else {
                            Ok(Some(
                                data.value
                                    .into_iter()
                                    .map(char::from)
                                    .collect::<String>()
                                    .into_bytes(),
                            ))
                        };
                    }
                    Some(_) => {}
                    None => thread::sleep(Duration::from_millis(4)),
                }
            }
        }
        // Like main's paste_text(), a non-text clipboard must not block input.
        Ok(None)
    }
    fn own(&self) -> Result<(), String> {
        self.conn
            .set_selection_owner(self.window, self.clipboard, CURRENT_TIME)
            .map_err(|e| e.to_string())?
            .check()
            .map_err(|e| e.to_string())?;
        self.conn.flush().map_err(|e| e.to_string())?;
        if self
            .conn
            .get_selection_owner(self.clipboard)
            .map_err(|e| e.to_string())?
            .reply()
            .map_err(|e| e.to_string())?
            .owner
            != self.window
        {
            return Err("게임 입력용 클립보드를 연결하지 못했습니다.".into());
        }
        Ok(())
    }
    fn restore_owner(&mut self) -> Result<(), String> {
        if self
            .conn
            .get_selection_owner(self.clipboard)
            .map_err(|e| e.to_string())?
            .reply()
            .map_err(|e| e.to_string())?
            .owner
            != self.window
        {
            return Err("입력 중 변경된 클립보드는 그대로 유지했습니다.".into());
        }
        let window = self.conn.generate_id().map_err(|e| e.to_string())?;
        let root = self
            .conn
            .get_geometry(self.window)
            .map_err(|e| e.to_string())?
            .reply()
            .map_err(|e| e.to_string())?
            .root;
        self.conn
            .create_window(
                COPY_DEPTH_FROM_PARENT,
                window,
                root,
                0,
                0,
                1,
                1,
                0,
                xproto::WindowClass::INPUT_OUTPUT,
                0,
                &xproto::CreateWindowAux::new(),
            )
            .map_err(|e| e.to_string())?
            .check()
            .map_err(|e| e.to_string())?;
        let previous_window = self.window;
        self.window = window;
        self.own()?;
        self.conn
            .destroy_window(previous_window)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    fn reply(
        &self,
        request: xproto::SelectionRequestEvent,
        content: &[u8],
    ) -> Result<bool, String> {
        let property = if request.property == NONE {
            request.target
        } else {
            request.property
        };
        let is_text = [
            self.utf8,
            self.plain,
            self.text,
            xproto::AtomEnum::STRING.into(),
        ]
        .contains(&request.target);
        let accepted = if request.selection != self.clipboard {
            false
        } else if request.target == self.targets {
            self.conn
                .change_property32(
                    PropMode::REPLACE,
                    request.requestor,
                    property,
                    xproto::AtomEnum::ATOM,
                    &[
                        self.targets,
                        self.utf8,
                        self.plain,
                        self.text,
                        xproto::AtomEnum::STRING.into(),
                    ],
                )
                .map_err(|e| e.to_string())?
                .check()
                .map_err(|e| e.to_string())?;
            true
        } else if is_text {
            let latin: Vec<u8>;
            let bytes = if request.target == u32::from(xproto::AtomEnum::STRING) {
                latin = String::from_utf8_lossy(content)
                    .chars()
                    .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
                    .collect();
                latin.as_slice()
            } else {
                content
            };
            self.conn
                .change_property8(
                    PropMode::REPLACE,
                    request.requestor,
                    property,
                    request.target,
                    bytes,
                )
                .map_err(|e| e.to_string())?
                .check()
                .map_err(|e| e.to_string())?;
            true
        } else {
            false
        };
        let event = SelectionNotifyEvent {
            response_type: xproto::SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time: request.time,
            requestor: request.requestor,
            selection: request.selection,
            target: request.target,
            property: if accepted { property } else { NONE },
        };
        self.conn
            .send_event(false, request.requestor, EventMask::NO_EVENT, event)
            .map_err(|e| e.to_string())?;
        self.conn.flush().map_err(|e| e.to_string())?;
        Ok(accepted && is_text)
    }
}

fn serve(
    text: Vec<u8>,
    ready: &Sender<Result<(), String>>,
    commands: Receiver<Sender<Result<(), String>>>,
) -> Result<(), String> {
    let mut selection = Selection::new()?;
    let previous = selection.previous()?;
    let mut content = text;
    selection.own()?;
    if ready.send(Ok(())).is_err() {
        return Ok(());
    }
    let mut pasted = false;
    let mut restore: Option<(Sender<Result<(), String>>, Instant)> = None;
    let mut restored = false;
    loop {
        match commands.try_recv() {
            Ok(reply) => restore = Some((reply, Instant::now())),
            Err(mpsc::TryRecvError::Disconnected) if !restored && restore.is_none() => {
                if let Some(previous) = &previous {
                    content = previous.clone();
                    selection.restore_owner()?;
                }
                restored = true;
            }
            _ => {}
        }
        while let Some(event) = selection.conn.poll_for_event().map_err(|e| e.to_string())? {
            match event {
                Event::SelectionRequest(request) => {
                    pasted |= selection.reply(request, &content)?;
                }
                Event::SelectionClear(event) if event.owner == selection.window => {
                    if let Some((reply, _)) = restore.take() {
                        let _ = reply.send(Err("입력 중 클립보드 내용이 변경됐습니다.".into()));
                    }
                    return Ok(());
                }
                _ => {}
            }
        }
        if restore
            .as_ref()
            .is_some_and(|(_, at)| pasted || at.elapsed() >= Duration::from_millis(800))
        {
            let (reply, _) = restore.take().unwrap();
            // A different owner window notifies XWayland clipboard bridges that
            // the contents have changed back, while keeping text available.
            let ownership = if let Some(previous) = &previous {
                content = previous.clone();
                selection.restore_owner()
            } else {
                Ok(())
            };
            let _ = reply.send(ownership.and_then(|()| {
                if pasted {
                    Ok(())
                } else {
                    Err("게임이 붙여넣기 내용을 요청하지 않았습니다.".into())
                }
            }));
            restored = true;
        }
        thread::sleep(Duration::from_millis(4));
    }
}
