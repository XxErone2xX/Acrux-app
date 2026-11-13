use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        mpsc::{self, Receiver, SyncSender},
        Arc, Mutex,
    },
};

const LIMIT: u64 = 5_000_000;
const HEADER: &str = "time,marker,zone_start,zone_end,velocity,click,bar_width\n";

enum Entry {
    Start(f64),
    Row {
        at: f64,
        marker: Option<f64>,
        zone: Option<(f64, f64)>,
        velocity: f64,
        click: bool,
        width: f64,
    },
}

pub(super) struct ReelLog {
    folder: PathBuf,
    sender: SyncSender<Entry>,
    error: Arc<Mutex<Option<String>>>,
    recording: std::cell::Cell<bool>,
}

impl ReelLog {
    pub(super) fn new(folder: &Path) -> Self {
        let folder = folder.to_owned();
        let path = folder.join("fishing_log.csv");
        let (sender, receiver) = mpsc::sync_channel(256);
        let error = Arc::new(Mutex::new(None));
        let report = error.clone();
        // Disk writes never run while the capture/input runtime is locked.
        std::thread::spawn(move || {
            if let Err(reason) = write_entries(&path, receiver) {
                *report.lock().unwrap() = Some(format!("릴링 기록 저장 실패: {reason}"));
            }
        });
        Self {
            folder,
            sender,
            error,
            recording: std::cell::Cell::new(false),
        }
    }

    pub(super) fn folder(&self) -> &Path {
        &self.folder
    }
    pub(super) fn error(&mut self) -> Option<String> {
        self.error.lock().unwrap().take()
    }
    fn send(&self, entry: Entry) -> bool {
        if let Err(reason) = self.sender.try_send(entry) {
            let message = match reason {
                mpsc::TrySendError::Full(_) => {
                    "릴링 기록 저장이 지연되어 일부 행을 건너뛰었습니다."
                }
                mpsc::TrySendError::Disconnected(_) => "릴링 기록 파일에 쓸 수 없습니다.",
            };
            self.error
                .lock()
                .unwrap()
                .get_or_insert_with(|| message.into());
            return false;
        }
        true
    }
    pub(super) fn start(&self, at: f64) {
        self.recording.set(self.send(Entry::Start(at)));
    }
    pub(super) fn row(
        &self,
        at: f64,
        marker: Option<f64>,
        zone: Option<(f64, f64)>,
        velocity: f64,
        click: bool,
        width: f64,
    ) {
        if !self.recording.get() {
            return;
        }
        self.send(Entry::Row {
            at,
            marker,
            zone,
            velocity,
            click,
            width,
        });
    }
}

fn write_entries(path: &Path, receiver: Receiver<Entry>) -> std::io::Result<()> {
    let mut file: Option<File> = None;
    let mut started = 0.0;
    for entry in receiver {
        match entry {
            Entry::Start(at) => {
                file.take();
                if path.metadata().is_ok_and(|m| m.len() > LIMIT) {
                    std::fs::remove_file(path)?;
                }
                let mut opened = OpenOptions::new().create(true).append(true).open(path)?;
                if opened.metadata()?.len() == 0 {
                    opened.write_all(HEADER.as_bytes())?;
                }
                writeln!(opened, "# reel {}", chrono::Utc::now().to_rfc3339())?;
                file = Some(opened);
                started = at;
            }
            Entry::Row {
                at,
                marker,
                zone,
                velocity,
                click,
                width,
            } => {
                if let Some(file) = &mut file {
                    let marker = marker.map(|m| format!("{m:.1}")).unwrap_or_default();
                    let (left, right) = zone
                        .map(|(a, b)| (format!("{a:.1}"), format!("{b:.1}")))
                        .unwrap_or_default();
                    writeln!(
                        file,
                        "{:.3},{marker},{left},{right},{velocity:.0},{},{width:.0}",
                        at - started,
                        u8::from(click)
                    )?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_observed_rows_and_resets_oversized_history_between_reels() {
        let path = std::env::temp_dir().join(format!("acrux-reel-{}.csv", uuid::Uuid::new_v4()));
        File::create(&path).unwrap().set_len(LIMIT + 1).unwrap();
        let (sender, receiver) = mpsc::sync_channel(8);
        sender.send(Entry::Start(41.0)).unwrap();
        sender
            .send(Entry::Row {
                at: 41.125,
                marker: Some(12.5),
                zone: Some((20.0, 40.0)),
                velocity: -80.0,
                click: true,
                width: 160.0,
            })
            .unwrap();
        sender
            .send(Entry::Row {
                at: 41.25,
                marker: None,
                zone: None,
                velocity: 0.0,
                click: false,
                width: 160.0,
            })
            .unwrap();
        drop(sender);
        write_entries(&path, receiver).unwrap();
        let output = std::fs::read_to_string(&path).unwrap();
        assert!(output.starts_with(HEADER));
        assert!(output.contains("0.125,12.5,20.0,40.0,-80,1,160\n"));
        assert!(output.contains("0.250,,,,0,0,160\n"));
        assert!(!output.contains('\0'));
        std::fs::remove_file(path).unwrap();
    }
}
