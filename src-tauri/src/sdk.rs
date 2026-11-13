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
    Inspect {
        ticket: u64,
        method: String,
        rect: Option<Rect>,
        pixels: Option<[usize; 4]>,
        reply: mpsc::Sender<Result<Value, String>>,
        mark: Option<String>,
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
impl Request {
    fn ticket(&self) -> u64 {
        match self {
            Self::Inspect { ticket, .. } | Self::Input { ticket, .. } => *ticket,
        }
    }
    pub(crate) fn cancel(self, error: &str) {
        let reply = match self {
            Self::Inspect { reply, .. } | Self::Input { reply, .. } => reply,
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
    if !["macro", "core"].contains(&module) {
        return Err("이 SDK 모듈은 native 명령을 제공하지 않습니다.".into());
    }
    let args = payload["args"].as_array().cloned().unwrap_or_default();
    let arg = |i: usize| args.get(i).cloned().unwrap_or(Value::Null);
    let kw = &payload["kwargs"];
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
    match method.as_str() {
        "ocr_boxes" | "ocr_region" | "ocr_inventory" | "fishing_check" | "client_rect" | "grab"
        | "waypoint_mark" => {
            if method == "waypoint_mark" && state.active() {
                return Err("현재 위치를 등록하기 전에 작업을 중지하세요.".into());
            }
            let region = if ["grab", "waypoint_mark", "client_rect", "fishing_check"]
                .contains(&method.as_str())
            {
                None
            } else if arg(0).is_null() {
                None
            } else {
                Some(rect(&arg(0))?)
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
            let mark = if method == "waypoint_mark" {
                Some(
                    arg(0)
                        .as_str()
                        .ok_or("등록할 목적지가 없습니다.")?
                        .to_owned(),
                )
            } else {
                None
            };
            state.sdk_request = Some(Request::Inspect {
                ticket,
                method,
                rect: region,
                pixels,
                reply: tx,
                mark,
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
    match rx.recv_timeout(Duration::from_secs(25)) {
        Ok(result) => result,
        Err(_) => {
            let mut state = runtime();
            if state
                .sdk_request
                .as_ref()
                .is_some_and(|r| r.ticket() == ticket)
            {
                if let Some(request) = state.sdk_request.take() {
                    request.cancel("SDK 작업 시간이 초과됐습니다.");
                }
                if let Some(runner) = state.automation.as_mut() {
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
        mark,
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
        if let Some(destination) = mark {
            let destination = match destination.as_str() {
                "base" => crate::navigator::Destination::Base,
                "coast" => crate::navigator::Destination::Coast,
                "sell" => crate::navigator::Destination::Sell,
                "craft" => crate::navigator::Destination::Craft,
                "match" => crate::navigator::Destination::Match,
                _ => return Err("목적지를 확인하세요.".into()),
            };
            let settings = state.movement_settings();
            state.wayfinding.memory = crate::wayfinder::Wayfinder::mark(
                destination,
                settings,
                geometry,
                frame,
                &state.ocr_words,
            )?;
            state.save(state.fishing.tuning)?;
            return Ok(json!({"ok":true,"message":"현재 관측에서 목적지 기준점을 저장했습니다."}));
        }
        if method == "grab" {
            let [x, y, w, h] = pixels.unwrap_or([0, 0, geometry.0, geometry.1]);
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
            Ok(
                json!({"__bgra__":base64::engine::general_purpose::STANDARD.encode(data),"width":w,"height":h}),
            )
        } else {
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
                Ok(json!(words
                    .iter()
                    .map(|w| json!([
                        w.text,
                        (w.rect.0 + w.rect.2) / 2.,
                        (w.rect.1 + w.rect.3) / 2.,
                        w.rect.2 - w.rect.0,
                        w.rect.3 - w.rect.1
                    ]))
                    .collect::<Vec<_>>()))
            }
        }
    })();
    let _ = reply.send(result);
    state.ocr_words.clear();
    Some(DeviceTick::Wait)
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
    fn screenshot_rpc_returns_actual_requested_bgra_and_stop_cancels_waiter() {
        let (mut state, (tx, rx)) = (Runtime::default(), mpsc::channel());
        state.sdk_request = Some(Request::Inspect {
            ticket: 1,
            method: "grab".into(),
            rect: None,
            pixels: Some([1, 0, 1, 2]),
            reply: tx,
            mark: None,
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
            mark: None,
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
