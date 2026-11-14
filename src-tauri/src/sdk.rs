//! Compatibility RPCs share the native observation and acknowledged input owner.
use crate::{
    automation::{self, Command, Job},
    now, runtime,
    vision::{Frame, Point, Rect},
    DeviceTick, Runtime,
};
use base64::Engine;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::Duration;

pub(crate) enum Request {
    Mark {
        ticket: u64,
        reply: mpsc::Sender<Result<Value, String>>,
    },
    Inspect {
        ticket: u64,
        method: String,
        rect: Option<Rect>,
        pixels: Option<[usize; 4]>,
        reply: mpsc::Sender<Result<Value, String>>,
    },
    Input {
        ticket: u64,
        command: Value,
        reply: mpsc::Sender<Result<Value, String>>,
        seen: bool,
    },
}
pub(crate) fn engine_name() -> &'static str {
    if cfg!(target_os = "android") {
        "ML Kit"
    } else if cfg!(target_os = "linux") {
        "Tesseract"
    } else if cfg!(target_os = "windows") {
        "Windows.Media.Ocr"
    } else {
        "Apple Vision"
    }
}

/// One public feature contract for every device adapter. Readiness and the
/// foreground/background policy are reported by status, not by hiding APIs.
pub(crate) fn capabilities() -> Value {
    json!({
        "apiVersion": 1,
        "platforms": ["windows", "linux", "macos", "android"],
        "routes": ["headless", "screenshot"],
        "features": ["mfish", "mpop", "mitem", "mmerch", "mcraft", "mmatch", "mblood"],
        "destinations": ["base", "coast", "sell", "craft", "match"],
        "methods": ["capabilities", "status", "logs", "start", "stop", "go", "mark", "cancel",
            "observe", "tap", "click", "key_tap", "gesture", "text", "open_link",
            "grab", "ocr_region", "ocr_boxes", "client_rect", "ocr_engine"],
        "coordinateSpace": "game-normalized",
        "pixelFormat": "BGRA",
        "clock": "app-monotonic-seconds",
        "ai": false
    })
}
impl Request {
    fn ticket(&self) -> u64 {
        match self {
            Self::Mark { ticket, .. }
            | Self::Inspect { ticket, .. }
            | Self::Input { ticket, .. } => *ticket,
        }
    }
    pub(crate) fn cancel(self, error: &str) {
        let reply = match self {
            Self::Mark { reply, .. } | Self::Inspect { reply, .. } | Self::Input { reply, .. } => {
                reply
            }
        };
        let _ = reply.send(Err(error.into()));
    }
}

fn rect(value: &Value) -> Result<Rect, String> {
    let a = value
        .as_array()
        .filter(|a| a.len() == 4)
        .ok_or("영역은 [left,top,right,bottom] 형식이어야 합니다.")?;
    let r = Rect(
        a[0].as_f64().unwrap_or(f64::NAN),
        a[1].as_f64().unwrap_or(f64::NAN),
        a[2].as_f64().unwrap_or(f64::NAN),
        a[3].as_f64().unwrap_or(f64::NAN),
    );
    if ![r.0, r.1, r.2, r.3]
        .into_iter()
        .all(|v| v.is_finite() && (0.0..=1.0).contains(&v))
        || r.2 <= r.0
        || r.3 <= r.1
    {
        return Err("영역이 게임 화면 밖이거나 비어 있습니다.".into());
    }
    Ok(r)
}

pub(crate) fn call(payload: Value, generation: i32) -> Result<Value, String> {
    let method = payload["method"]
        .as_str()
        .ok_or("SDK 함수가 없습니다.")?
        .to_owned();
    let module = payload["module"].as_str().unwrap_or("macro");
    if !["dua", "macro", "core"].contains(&module) {
        return Err("이 SDK 모듈은 native 명령을 제공하지 않습니다.".into());
    }
    let args = payload["args"].as_array().cloned().unwrap_or_default();
    let arg = |i: usize| args.get(i).cloned().unwrap_or(Value::Null);
    let kw = &payload["kwargs"];
    if module == "dua" {
        match method.as_str() {
            "capabilities" => return Ok(capabilities()),
            "status" => return serde_json::to_value(crate::status()).map_err(|e| e.to_string()),
            "logs" => return Ok(json!(crate::fishing_log())),
            _ => {}
        }
        // Do not hold Runtime while entering the existing public controllers.
        // In particular stop/cancel must preempt a pending capture or input RPC.
        if generation != runtime().generation {
            return Err("이전 SDK 실행이 취소됐습니다.".into());
        }
        match method.as_str() {
            "start" => {
                let value = if args.is_empty() {
                    kw["features"].clone()
                } else {
                    arg(0)
                };
                let features: Vec<String> = serde_json::from_value(value)
                    .map_err(|_| "실행할 기능 ID 목록을 전달하세요.".to_owned())?;
                return crate::dua_start(features, generation)
                    .and_then(|value| serde_json::to_value(value).map_err(|e| e.to_string()));
            }
            "stop" => return serde_json::to_value(crate::dua_stop()).map_err(|e| e.to_string()),
            "go" | "mark" | "cancel" => {
                let action = match method.as_str() {
                    "go" => "go",
                    "mark" => "waypoint_mark",
                    _ => "waypoint_cancel",
                };
                let destination = if args.is_empty() {
                    kw["destination"].clone()
                } else {
                    arg(0)
                };
                return crate::automation_call(
                    action.into(),
                    json!({"destination":destination}),
                    generation,
                );
            }
            _ => {}
        }
    }
    let mut state = runtime();
    if generation != state.generation {
        return Err("이전 SDK 실행이 취소됐습니다.".into());
    }
    match method.as_str() {
        "ocr_engine"|"ocr_engine_name"=>return Ok(json!(if cfg!(target_os="android"){"ML Kit"}else if cfg!(target_os="linux"){"Tesseract"}else if cfg!(target_os="windows"){"Windows.Media.Ocr"}else{"Apple Vision"})),
        "roblox_window"|"roblox_window_cached"|"focus"=>return Err("전역 창 핸들과 전경 전환은 사용할 수 없습니다. native SDK 입력은 연결된 게임 대상을 자동으로 사용합니다.".into()),
        _=>{}
    }
    if state.sdk_request.is_some() {
        return Err("이전 SDK 작업의 완료를 기다리세요.".into());
    }
    if !state.capture || !state.input {
        return Err("게임 캡처와 입력을 먼저 연결하세요.".into());
    }
    let (tx, rx) = mpsc::channel();
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let ticket = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let timeout = Duration::from_secs(if method == "waypoint_mark" { 60 } else { 25 });
    match method.as_str() {
        "waypoint_mark" => {
            if state.active() {
                return Err("현재 위치를 등록하기 전에 작업을 중지하세요.".into());
            }
            let destination = serde_json::from_value(arg(0))
                .map_err(|_| "등록할 목적지를 확인하세요.".to_owned())?;
            state.wayfinder = Some(crate::wayfinder::Wayfinder::start_mark(
                destination,
                state.movement_settings(),
                now(),
            )?);
            // Registration uses the same camera preparation and acknowledged
            // input path as travel. A single raw inspection can save a map from
            // a different camera pitch and cannot be matched on the next trip.
            state.dua.running = true;
            state.generation = state.generation.wrapping_add(1);
            state.phone.cancel();
            state.sdk_request = Some(Request::Mark { ticket, reply: tx });
            state.macro_message = Some("카메라를 위로 올려 현재 장소의 지형을 기억합니다.".into());
            state.record();
        }
        "observe" | "ocr_boxes" | "ocr_region" | "ocr_inventory" | "fishing_check"
        | "client_rect" | "grab" => {
            let region = if ["grab", "client_rect", "fishing_check"].contains(&method.as_str()) {
                None
            } else if arg(0).is_null() && kw["rect"].is_null() {
                None
            } else {
                Some(rect(&if arg(0).is_null() {
                    kw["rect"].clone()
                } else {
                    arg(0)
                })?)
            };
            let pixels = if method == "grab" {
                let a = arg(0);
                let a = a
                    .as_array()
                    .filter(|a| a.len() == 4)
                    .ok_or("grab에는 게임 기준 [left,top,width,height]가 필요합니다.")?;
                let mut p = [0; 4];
                for (i, v) in a.iter().enumerate() {
                    p[i] = v
                        .as_u64()
                        .and_then(|n| usize::try_from(n).ok())
                        .ok_or("잘못된 픽셀 영역입니다.")?;
                }
                Some(p)
            } else {
                None
            };
            state.sdk_request = Some(Request::Inspect {
                ticket,
                method,
                rect: region,
                pixels,
                reply: tx,
            });
        }
        _ => {
            let number = |i, default| args.get(i).and_then(Value::as_f64).unwrap_or(default);
            let ms = |i, default| {
                kw["hold_ms"]
                    .as_u64()
                    .or_else(|| args.get(i).and_then(Value::as_u64))
                    .unwrap_or(default)
                    .clamp(1, 5000) as u32
            };
            let command=match method.as_str(){
                "gesture"=>Command::Gesture(serde_json::from_value(arg(0)).map_err(|e|format!("gesture 형식을 확인하세요: {e}"))?),
                "tap"|"click"=>{
                    let(mut x,mut y)=(number(0,f64::NAN),number(1,f64::NAN));
                    if method=="click" {let(w,h)=state.last_geometry.ok_or("관측한 게임 크기가 없습니다.")?;x/=w as f64;y/=h as f64;}
                    if kw["button"].as_str().or_else(||args.get(2).and_then(Value::as_str)).is_some_and(|s|s!="left"){return Err("오른쪽 클릭은 gesture drag API를 사용하세요.".into());}
                    let gesture=crate::device::Gesture::Tap{at:Point(x,y),ms:ms(if method=="tap"{2}else{3},40)};
                    Command::Gesture(gesture)
                }
                "key_tap"=>{
                    let name=arg(0).as_str().unwrap_or("").to_ascii_lowercase();
                    let vk=match name.as_str(){"space"=>0x20,"enter"=>0x0d,"esc"|"escape"=>0x1b,"tab"=>0x09,"backspace"=>0x08,"/"=>0xbf,_ if name.len()==1&&name.as_bytes()[0].is_ascii_alphanumeric()=>u16::from(name.as_bytes()[0].to_ascii_uppercase()),_=>return Err("지원하지 않는 키 이름입니다.".into())};
                    Command::Key{vk,ms:ms(1,40)}
                }
                "type_text"|"text"|"paste"=>Command::Text{text:arg(0).as_str().ok_or("입력할 문자열이 없습니다.")?.into(),submit:kw["submit"].as_bool().unwrap_or(false)},
                "open_link"=>Command::Launch{url:arg(0).as_str().ok_or("게임 접속 주소가 없습니다.")?.into()},
                _=>return Err(format!("지원하지 않는 구형 SDK 함수: {module}.{method}. 입력은 tap/key_tap/text/gesture, 관측은 grab/ocr_boxes/ocr_region을 사용하세요.")),
            };
            let encoded = serde_json::to_value(&command).map_err(|e| e.to_string())?;
            if state.automation.is_none() {
                if state.active() {
                    return Err("다른 도구가 입력을 사용하고 있습니다.".into());
                }
                let mut runner = automation::Runner::new(state.macro_config(), now())?;
                runner.restore_session(&state.last_automation);
                runner.queue(Job::Command { command })?;
                state.automation = Some(runner);
                state.dua.running = true;
            } else {
                state
                    .automation
                    .as_mut()
                    .unwrap()
                    .queue(Job::Command { command })?;
            }
            state.sdk_request = Some(Request::Input {
                ticket,
                command: encoded,
                reply: tx,
                seen: false,
            });
        }
    }
    drop(state);
    match rx.recv_timeout(timeout) {
        Ok(result) => result,
        Err(_) => {
            let mut state = runtime();
            if state
                .sdk_request
                .as_ref()
                .is_some_and(|r| r.ticket() == ticket)
            {
                let marking = matches!(state.sdk_request, Some(Request::Mark { .. }));
                if let Some(request) = state.sdk_request.take() {
                    request.cancel("SDK 작업 시간이 초과됐습니다.");
                }
                if marking {
                    state.stop("장소 등록 시간이 초과돼 입력을 중지했습니다.");
                } else if let Some(runner) = state.automation.as_mut() {
                    runner.stop_feature("sdk");
                }
                state.generation = state.generation.wrapping_add(1);
                state.phone.cancel();
            }
            Err("SDK 작업 시간이 초과됐습니다. 미확인 입력을 반복하지 않습니다.".into())
        }
    }
}

pub(crate) fn inspect(
    state: &mut Runtime,
    geometry: (usize, usize),
    frame: Option<&Frame<'_>>,
) -> Option<DeviceTick> {
    let Some(Request::Inspect { .. }) = state.sdk_request.as_ref() else {
        return None;
    };
    state.request_ocr = matches!(state.sdk_request.as_ref(),Some(Request::Inspect{method,..})if !["grab","fishing_check","client_rect"].contains(&method.as_str()));
    let Some(frame) = frame else {
        return Some(DeviceTick::Observe(None));
    };
    let Some(Request::Inspect {
        method,
        rect,
        pixels,
        reply,
        ..
    }) = state.sdk_request.take()
    else {
        unreachable!()
    };
    let result = (|| {
        if method == "client_rect" {
            return Ok(json!([0, 0, geometry.0, geometry.1]));
        }
        if method == "fishing_check" {
            return state.fishing.check(frame);
        }
        let words: Vec<_> = state
            .ocr_words
            .iter()
            .filter(|w| {
                rect.is_none_or(|r| {
                    let x = (w.rect.0 + w.rect.2) / 2.;
                    let y = (w.rect.1 + w.rect.3) / 2.;
                    x >= r.0 && x <= r.2 && y >= r.1 && y <= r.3
                })
            })
            .collect();
        let boxes = || {
            words
                .iter()
                .map(|w| {
                    json!([
                        w.text,
                        (w.rect.0 + w.rect.2) / 2.,
                        (w.rect.1 + w.rect.3) / 2.,
                        w.rect.2 - w.rect.0,
                        w.rect.3 - w.rect.1
                    ])
                })
                .collect::<Vec<_>>()
        };
        if method == "grab" || method == "observe" {
            let [x, y, w, h] = if method == "observe" {
                if let Some(rect) = rect {
                    let r = rect
                        .region(geometry)
                        .ok_or("관측 영역이 게임 화면 밖입니다.")?;
                    [r.x, r.y, r.width, r.height]
                } else {
                    [0, 0, geometry.0, geometry.1]
                }
            } else {
                pixels.unwrap_or([0, 0, geometry.0, geometry.1])
            };
            if w == 0
                || h == 0
                || x.checked_add(w).is_none_or(|v| v > geometry.0)
                || y.checked_add(h).is_none_or(|v| v > geometry.1)
                || w.checked_mul(h).is_none_or(|v| v > 3_000_000)
            {
                return Err("스크린샷 영역이 비어 있거나 너무 큽니다.".into());
            }
            let mut data = Vec::with_capacity(w * h * 4);
            for yy in y..y + h {
                for xx in x..x + w {
                    let [r, g, b] = frame
                        .pixel(xx, yy)
                        .ok_or("요청 영역의 실제 픽셀이 없습니다.")?;
                    data.extend_from_slice(&[b, g, r, 255]);
                }
            }
            let mut image = json!({"__bgra__":base64::engine::general_purpose::STANDARD.encode(data),"width":w,"height":h});
            if method == "observe" {
                let captured_at = state
                    .frame_captured
                    .filter(|v| v.is_finite())
                    .ok_or("관측한 프레임의 촬영 시각이 없습니다.")?;
                image["origin"] = json!([x, y]);
                Ok(
                    json!({"image":image,"words":boxes(),"capturedAt":captured_at,
                    "inputDeadline":state.frame_expires,"generation":state.generation,"engine":engine_name(),
                    "viewport":[geometry.0,geometry.1]}),
                )
            } else {
                Ok(image)
            }
        } else {
            if method == "ocr_inventory" {
                let mut result = crate::automation::inventory_text(
                    &words
                        .iter()
                        .map(|w| w.text.as_str())
                        .collect::<Vec<_>>()
                        .join("\n"),
                );
                result["engine"] = json!(engine_name());
                Ok(result)
            } else if method == "ocr_region" {
                Ok(json!(words
                    .iter()
                    .map(|w| w.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")))
            } else {
                Ok(json!(boxes()))
            }
        }
    })();
    let _ = reply.send(result);
    state.ocr_words.clear();
    Some(DeviceTick::Wait)
}

/// Resolve the registration RPC after the shared camera/input controller has
/// captured a settled overhead map (or stopped). Ordinary travel has no waiter.
pub(crate) fn finish_mark(state: &mut Runtime, result: Result<(), String>) -> bool {
    if !matches!(state.sdk_request, Some(Request::Mark { .. })) {
        return false;
    }
    let Some(Request::Mark { reply, .. }) = state.sdk_request.take() else {
        unreachable!()
    };
    state.stop("현재 장소의 지형을 기억했습니다.");
    if let Err(error) = &result {
        state.macro_message = Some(error.clone());
    }
    state.record();
    let _ = reply
        .send(result.map(|()| json!({"ok":true,"message":"현재 장소의 지형을 기억했습니다."})));
    true
}

pub(crate) fn before_tick(state: &mut Runtime) {
    if let (Some(Request::Input { command, seen, .. }), Some(runner)) =
        (&mut state.sdk_request, &state.automation)
    {
        if let Some(Job::Command { command: active }) = &runner.status().active {
            if serde_json::to_value(active).ok().as_ref() == Some(command) {
                *seen = true;
            }
        }
    }
}
pub(crate) fn after_tick(state: &mut Runtime, done: bool, failed: bool) {
    if done && matches!(state.sdk_request, Some(Request::Input { seen: true, .. })) {
        if let Some(Request::Input { reply, .. }) = state.sdk_request.take() {
            let _ = reply.send(if failed {
                Err("입력 완료를 확인하지 못했습니다.".into())
            } else {
                Ok(json!(true))
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dua_public_contract_and_read_queries_do_not_require_a_game_or_fresh_input_generation() {
        let capabilities = call(json!({"module":"dua","method":"capabilities"}), i32::MIN).unwrap();
        assert_eq!(capabilities["apiVersion"], 1);
        assert_eq!(
            capabilities["platforms"],
            json!(["windows", "linux", "macos", "android"])
        );
        assert_eq!(capabilities["routes"], json!(["headless", "screenshot"]));
        assert_eq!(
            capabilities["features"],
            json!(["mfish", "mpop", "mitem", "mmerch", "mcraft", "mmatch", "mblood"])
        );
        assert_eq!(capabilities["ai"], false);
        let methods = capabilities["methods"].as_array().unwrap();
        for method in [
            "observe", "go", "mark", "cancel", "start", "stop", "tap", "gesture",
        ] {
            assert!(methods.contains(&json!(method)));
        }
        assert!(
            call(json!({"module":"dua","method":"status"}), i32::MIN).unwrap()["generation"]
                .is_number()
        );
        assert!(call(json!({"module":"dua","method":"logs"}), i32::MIN)
            .unwrap()
            .is_array());
        for method in ["start", "stop", "go", "mark", "cancel", "tap", "observe"] {
            let error =
                call(json!({"module":"dua","method":method,"args":[]}), i32::MIN).unwrap_err();
            assert!(error.contains("취소"), "{method}: {error}");
        }
    }

    #[test]
    fn observation_returns_pixels_words_crop_and_original_capture_time_from_one_frame() {
        let (tx, rx) = mpsc::channel();
        let mut state = Runtime {
            generation: 7,
            frame_captured: Some(1.25),
            frame_expires: Some(2.3),
            ..Runtime::default()
        };
        state.sdk_request = Some(Request::Inspect {
            ticket: 3,
            method: "observe".into(),
            rect: Some(Rect(0.25, 0., 0.75, 1.)),
            pixels: None,
            reply: tx,
        });
        assert!(matches!(
            inspect(&mut state, (4, 2), None),
            Some(DeviceTick::Observe(None))
        ));
        assert!(state.request_ocr);
        state.ocr_words = vec![
            automation::Word {
                text: "Fish".into(),
                rect: Rect(0.3, 0.1, 0.7, 0.5),
            },
            automation::Word {
                text: "outside".into(),
                rect: Rect(0.8, 0.1, 1., 0.5),
            },
        ];
        let bytes: Vec<u8> = (0..8).flat_map(|n| [n, n + 10, n + 20, 255]).collect();
        let frame = Frame::new(&bytes, 4, 2, 16, 4, false).unwrap();
        inspect(&mut state, (4, 2), Some(&frame));
        let observed = rx.recv().unwrap().unwrap();
        assert_eq!(observed["capturedAt"], 1.25);
        assert_eq!(observed["inputDeadline"], 2.3);
        assert_eq!(observed["generation"], 7);
        assert_eq!(observed["viewport"], json!([4, 2]));
        assert_eq!(observed["image"]["origin"], json!([1, 0]));
        assert_eq!(observed["image"]["width"], 2);
        assert_eq!(observed["image"]["height"], 2);
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(observed["image"]["__bgra__"].as_str().unwrap())
                .unwrap(),
            [21, 11, 1, 255, 22, 12, 2, 255, 25, 15, 5, 255, 26, 16, 6, 255]
        );
        assert_eq!(observed["words"].as_array().unwrap().len(), 1);
        assert_eq!(observed["words"][0][0], "Fish");
        assert_eq!(observed["words"][0][1], 0.5);
        assert!(state.ocr_words.is_empty());
    }

    #[test]
    fn place_registration_keeps_its_waiter_until_shared_camera_input_is_acknowledged() {
        let (tx, rx) = mpsc::channel();
        let mut state = Runtime {
            capture: true,
            input: true,
            ..Runtime::default()
        };
        state.wayfinder = Some(
            crate::wayfinder::Wayfinder::start_mark(
                crate::navigator::Destination::Sell,
                state.movement_settings(),
                0.,
            )
            .unwrap(),
        );
        state.dua.running = true;
        state.sdk_request = Some(Request::Mark {
            ticket: 4,
            reply: tx,
        });
        assert!(inspect(&mut state, (480, 270), None).is_none());
        let pixels = vec![60; 480 * 270 * 4];
        let frame = Frame::new(&pixels, 480, 270, 1920, 4, false).unwrap();
        assert!(matches!(
            crate::observed_tick(&mut state, (480, 270), &frame, Vec::new(), 0.1, 0.1, None),
            DeviceTick::Input(crate::GameInput::Command(Command::Gesture(
                crate::device::Gesture::Drag { .. }
            )))
        ));
        assert!(matches!(rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
        assert!(matches!(
            state.tick(0.2, (480, 270), None),
            DeviceTick::Wait
        ));
        assert!(matches!(rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
        state.device_ack_at(0.45);
        state.stop("등록 취소");
        assert_eq!(rx.recv().unwrap().unwrap_err(), "등록 취소");
        assert!(!state.dua.running && state.wayfinder.is_none());
    }

    #[test]
    fn failed_place_registration_remains_visible_in_status_and_log() {
        let (tx, rx) = mpsc::channel();
        let mut state = Runtime::default();
        state.sdk_request = Some(Request::Mark {
            ticket: 1,
            reply: tx,
        });
        state.dua.running = true;
        assert!(finish_mark(
            &mut state,
            Err("지형이 보이지 않습니다.".into())
        ));
        let error = rx.recv().unwrap().unwrap_err();
        assert_eq!(state.status().message, error);
        assert_eq!(state.log.back().unwrap().1, error);
        assert!(state.wayfinding.memory.anchors.is_empty());
        assert!(!state.dua.running && state.sdk_request.is_none());
    }
    #[test]
    fn screenshot_rpc_returns_actual_requested_bgra_and_stop_cancels_waiter() {
        let (mut state, (tx, rx)) = (Runtime::default(), mpsc::channel());
        state.sdk_request = Some(Request::Inspect {
            ticket: 1,
            method: "grab".into(),
            rect: None,
            pixels: Some([1, 0, 1, 2]),
            reply: tx,
        });
        assert!(matches!(
            inspect(&mut state, (2, 2), None),
            Some(DeviceTick::Observe(None))
        ));
        let bytes = [1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255];
        let frame = Frame::new(&bytes, 2, 2, 8, 4, false).unwrap();
        inspect(&mut state, (2, 2), Some(&frame));
        let reply = rx.recv().unwrap().unwrap();
        assert_eq!(reply["width"], 1);
        assert_eq!(reply["height"], 2);
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(reply["__bgra__"].as_str().unwrap())
                .unwrap(),
            [6, 5, 4, 255, 12, 11, 10, 255]
        );
        let (tx, rx) = mpsc::channel();
        state.sdk_request = Some(Request::Inspect {
            ticket: 2,
            method: "grab".into(),
            rect: None,
            pixels: None,
            reply: tx,
        });
        state.stop("중지됨");
        assert_eq!(rx.recv().unwrap().unwrap_err(), "중지됨");
    }
    #[test]
    fn command_rpc_waits_for_actual_ack_and_a_fresh_observation() {
        let command = Command::Key { vk: 0x45, ms: 30 };
        let mut runner = automation::Runner::new(json!({}), 0.).unwrap();
        runner
            .queue(Job::Command {
                command: command.clone(),
            })
            .unwrap();
        let (tx, rx) = mpsc::channel();
        let mut state = Runtime {
            capture: true,
            input: true,
            automation: Some(runner),
            ..Runtime::default()
        };
        state.dua.running = true;
        state.sdk_request = Some(Request::Input {
            ticket: 1,
            command: serde_json::to_value(command).unwrap(),
            reply: tx,
            seen: false,
        });
        let pixels = [60; 8 * 8 * 4];
        let frame = Frame::new(&pixels, 8, 8, 32, 4, false).unwrap();
        assert!(matches!(
            state.tick(0., (8, 8), None),
            DeviceTick::Input(crate::GameInput::Command(_))
        ));
        assert!(rx.try_recv().is_err());
        state.tick(0.01, (8, 8), None);
        assert!(rx.try_recv().is_err());
        state.device_ack_at(0.02);
        assert!(matches!(
            state.tick(0.03, (8, 8), None),
            DeviceTick::Observe(_)
        ));
        assert!(rx.try_recv().is_err());
        state.tick(0.04, (8, 8), Some(&frame));
        assert_eq!(rx.recv().unwrap().unwrap(), json!(true));
        assert!(!state.active());
    }
    #[test]
    fn outside_rectangles_are_rejected_instead_of_silently_clamped() {
        assert!(rect(&json!([-0.1, 0., 1., 1.])).is_err());
        assert!(rect(&json!([0., 0., 1.1, 1.])).is_err());
    }
}
