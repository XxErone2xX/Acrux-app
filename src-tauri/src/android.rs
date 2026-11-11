use crate::{
    runtime,
    vision::{Frame, Point},
    Action, GameInput, Motion, Place, Task,
};
use jni::{
    objects::{JByteBuffer, JClass, JObject, JString},
    sys::{jboolean, jdouble, jint, jintArray, jstring},
    JNIEnv,
};
use tauri::{
    plugin::{Builder, PluginHandle, TauriPlugin},
    Manager, Wry,
};

pub fn plugin() -> TauriPlugin<Wry> {
    Builder::new("acrux-android")
        .setup(|app, api| {
            app.manage(api.register_android_plugin("com.acrux.next", "AndroidSetup")?);
            Ok(())
        })
        .build()
}

pub fn setup(
    app: &tauri::AppHandle,
    action: &str,
    options: Option<serde_json::Value>,
) -> Result<(), String> {
    let command = match action {
        "accessibility" => "accessibility",
        "capture" => "capture",
        "stopCapture" => "stopCapture",
        "overlay" => "overlay",
        "appInfo" => "appInfo",
        "donate" => "donate",
        "biome" => "biome",
        _ => return Err("알 수 없는 준비 작업입니다.".into()),
    };
    app.state::<PluginHandle<Wry>>()
        .run_mobile_plugin::<()>(command, options)
        .map_err(|err| err.to_string())
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_capture(_: JNIEnv, _: JClass, ready: jboolean) {
    let mut state = runtime();
    state.capture = ready != 0;
    if !state.capture {
        state.stop("화면 캡처가 종료됐습니다.");
    }
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_accessibility(_: JNIEnv, _: JClass, ready: jboolean) {
    let mut state = runtime();
    state.input = ready != 0;
    if !state.input {
        state.stop("접근성 서비스 연결이 종료됐습니다.");
    }
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_active(_: JNIEnv, _: JClass) -> jboolean {
    runtime().phase().active().into()
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_inputCurrent(
    _: JNIEnv,
    _: JClass,
    generation: jint,
) -> jboolean {
    runtime().input_current(generation).into()
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_overlayPhase(_: JNIEnv, _: JClass) -> jint {
    match runtime().phase() {
        crate::Phase::Idle => 0,
        crate::Phase::Calibrating => 1,
        crate::Phase::Recording => 3,
        _ => 2,
    }
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_captureReady(_: JNIEnv, _: JClass) -> jboolean {
    runtime().capture.into()
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_control(
    env: JNIEnv,
    _: JClass,
    operation: jint,
) -> jstring {
    let action = match operation {
        0 => Action::Stop,
        1 => Action::Calibrate,
        2 => Action::Start,
        _ => return JObject::null().into_raw(),
    };
    match runtime().control(action) {
        Ok(()) => JObject::null().into_raw(),
        Err(error) => env
            .new_string(error)
            .map_or(std::ptr::null_mut(), |s| s.into_raw()),
    }
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_message(env: JNIEnv, _: JClass) -> jstring {
    let message = runtime().message();
    env.new_string(message)
        .map_or(std::ptr::null_mut(), |s| s.into_raw())
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_noTarget(_: JNIEnv, _: JClass) {
    runtime().no_target();
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_inputRejected(_: JNIEnv, _: JClass) {
    runtime().input_rejected("안드로이드 터치가 취소됐습니다. 조작 창 위치를 확인하세요.");
}

fn reply(env: &JNIEnv, result: Result<(), String>) -> jstring {
    result
        .err()
        .and_then(|error| env.new_string(error).ok())
        .map_or(std::ptr::null_mut(), |s| s.into_raw())
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_task(
    mut env: JNIEnv,
    _: JClass,
    task: JString,
) -> jstring {
    let name: String = match env.get_string(&task) {
        Ok(name) => name.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let result = serde_json::from_value::<Task>(name.into())
        .map_err(|_| "알 수 없는 이동 작업입니다.".to_owned())
        .and_then(|task| runtime().task(task).map_err(str::to_owned));
    reply(&env, result)
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_setPoint(
    mut env: JNIEnv,
    _: JClass,
    key: JString,
    x: jdouble,
    y: jdouble,
) -> jstring {
    let key: String = match env.get_string(&key) {
        Ok(key) => key.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let mut state = runtime();
    let result = if state.phase().active() {
        Err("실행 중인 작업을 먼저 중지하세요.".into())
    } else {
        state.edit_navigation(|nav| nav.set_point(&key, Point(x, y)))
    };
    reply(&env, result)
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_navigation(env: JNIEnv, _: JClass) -> jstring {
    env.new_string(crate::navigation_status().to_string())
        .map_or(std::ptr::null_mut(), |s| s.into_raw())
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_saveTrace(
    mut env: JNIEnv,
    _: JClass,
    trace: JString,
) -> jstring {
    let result = (|| {
        let json: String = env.get_string(&trace).map_err(|e| e.to_string())?.into();
        let events: Vec<crate::navigation::TraceEvent> =
            serde_json::from_str(&json).map_err(|e| e.to_string())?;
        let events = crate::navigation::normalize_trace(events)?;
        let mut state = runtime();
        let place = state
            .job
            .as_ref()
            .filter(|job| job.recording_at.is_some())
            .and_then(|job| job.action.record())
            .ok_or("길 기록 중이 아닙니다.")?;
        state.edit_navigation(|nav| {
            nav.route(place).clear();
            *nav.trace(place) = events;
            Ok(())
        })?;
        state.stop("길 기록을 저장했습니다.");
        Ok(())
    })();
    reply(&env, result)
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_clearRoute(
    env: JNIEnv,
    _: JClass,
    sell: jboolean,
) -> jstring {
    let mut state = runtime();
    let result = if state.phase().active() {
        Err("실행 중인 작업을 먼저 중지하세요.".into())
    } else {
        state.edit_navigation(|nav| {
            let place = if sell != 0 { Place::Sell } else { Place::Fish };
            nav.route(place).clear();
            nav.trace(place).clear();
            Ok(())
        })
    };
    reply(&env, result)
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_frame(
    env: JNIEnv,
    _: JClass,
    buffer: JByteBuffer,
    width: jint,
    height: jint,
    row_stride: jint,
    pixel_stride: jint,
) -> jintArray {
    if width <= 0
        || height <= 0
        || width >= 32768
        || height >= 32768
        || row_stride <= 0
        || pixel_stride <= 0
    {
        return std::ptr::null_mut();
    }
    let Ok(pointer) = env.get_direct_buffer_address(&buffer) else {
        return std::ptr::null_mut();
    };
    let Ok(capacity) = env.get_direct_buffer_capacity(&buffer) else {
        return std::ptr::null_mut();
    };
    // The caller owns the Image and keeps its plane open until this synchronous call returns.
    let bytes = unsafe { std::slice::from_raw_parts(pointer, capacity) };
    let Some(frame) = Frame::new(
        bytes,
        width as usize,
        height as usize,
        row_stride as usize,
        pixel_stride as usize,
        false,
    ) else {
        return std::ptr::null_mut();
    };
    let (input, generation) = {
        let mut state = runtime();
        let Some(input) = state.frame(&frame) else {
            return std::ptr::null_mut();
        };
        (input, state.generation)
    };
    let px = |point| {
        let (x, y) = frame.position(point);
        [x as i32, y as i32]
    };
    let [w, h] = [frame.width as f64, frame.height as f64];
    let mut values = match input {
        GameInput::Tap(tap) => vec![
            0,
            tap.duration.as_millis() as i32,
            tap.x as i32,
            tap.y as i32,
        ],
        GameInput::Motion(motion) => match motion {
            Motion::Trace {
                center,
                edge,
                jump,
                events,
            } => {
                let [cx, cy] = px(center);
                let [ex, ey] = px(edge);
                let [jx, jy] = jump.map_or([-1, -1], px);
                let mut packet = vec![
                    4,
                    events.last().map_or(0, |event| event.t.ceil() as i32),
                    cx,
                    cy,
                    ex,
                    ey,
                    jx,
                    jy,
                ];
                for event in events {
                    packet.extend([
                        event.k as i32,
                        (event.t * 10.0).round() as i32,
                        (event.dx * 1000.0).round() as i32,
                        (event.dy * 1000.0).round() as i32,
                        i32::from(event.a) | (i32::from(event.b) << 1),
                    ]);
                }
                packet
            }
            Motion::Tap(point, ms) => {
                let [x, y] = px(point);
                vec![0, ms as i32, x, y]
            }
            Motion::Keys(..) | Motion::Click { .. } | Motion::Tilt { .. } => {
                return std::ptr::null_mut()
            }
            Motion::Swipe => {
                let [x, y0] = px(Point(0.62, 0.22));
                let [_, y1] = px(Point(0.62, 0.80));
                vec![1, 250, x, y0, x, y1]
            }
            Motion::Pinch => {
                let [x, a] = px(Point(0.55, 0.20));
                let [_, b] = px(Point(0.55, 0.80));
                let [_, c] = px(Point(0.55, 0.47));
                let [_, d] = px(Point(0.55, 0.53));
                vec![2, 300, x, a, x, c, x, b, x, d]
            }
            Motion::Stick {
                center,
                edge,
                x,
                y,
                jump,
                ms,
            } => {
                let radius = ((edge.0 - center.0) * w).hypot((edge.1 - center.1) * h);
                let gain = radius * 1.35 / f64::from(x).hypot(f64::from(y)).max(1.0);
                let [x0, y0] = px(center);
                let [x1, y1] = px(Point(
                    center.0 + f64::from(x) * gain / w,
                    center.1 + f64::from(y) * gain / h,
                ));
                let [jx, jy] = jump.map_or([-1, -1], px);
                vec![3, ms as i32, x0, y0, x1, y1, jx, jy]
            }
        },
    };
    values.push(generation);
    let Ok(array) = env.new_int_array(values.len() as i32) else {
        return std::ptr::null_mut();
    };
    if env.set_int_array_region(&array, 0, &values).is_err() {
        return std::ptr::null_mut();
    }
    array.into_raw()
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_biomeStatus(env: JNIEnv, _: JClass) -> jstring {
    env.new_string(serde_json::to_string(&crate::biome::status()).unwrap_or_default())
        .map_or(std::ptr::null_mut(), |s| s.into_raw())
}
#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_biomeToggle(env: JNIEnv, _: JClass) -> jstring {
    let mut settings = crate::biome::settings();
    settings.enabled = !settings.enabled;
    reply(&env, crate::save_biome(settings).map(|_| ()))
}
#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_biomeLine(
    mut env: JNIEnv,
    _: JClass,
    line: JString,
    source: jint,
    snapshot: jboolean,
) {
    if let Ok(line) = env.get_string(&line) {
        crate::biome::line(&String::from(line), source, snapshot != 0);
    }
}
#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_biomeConnection(
    mut env: JNIEnv,
    _: JClass,
    ready: jboolean,
    message: JString,
) {
    if let Ok(message) = env.get_string(&message) {
        crate::biome::connection(ready != 0, message.into());
    }
}
#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_platformLog(
    mut env: JNIEnv,
    _: JClass,
    message: JString,
    _: jni::sys::jchar,
) {
    if let Ok(message) = env.get_string(&message) {
        crate::biome::platform_log(message.into());
    }
}

pub fn transport(app: &tauri::AppHandle) -> Result<serde_json::Value, String> {
    app.state::<PluginHandle<Wry>>()
        .run_mobile_plugin("biomeTransport", ())
        .map_err(|e| e.to_string())
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_generation(_: JNIEnv, _: JClass) -> jint {
    runtime().generation
}
