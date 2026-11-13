use crate::{
    runtime,
    vision::{Frame, Point},
    Action, Place, Task,
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
        "discordLink" => "discordLink",
        "biome" => "biome",
        "inputSetup" => "inputSetup",
        "serviceAction" => "serviceAction",
        "returnGame" => "returnGame",
        _ => return Err("알 수 없는 준비 작업입니다.".into()),
    };
    app.state::<PluginHandle<Wry>>()
        .run_mobile_plugin::<()>(command, options)
        .map_err(|err| err.to_string())
}

pub fn input_status(app: &tauri::AppHandle) -> Result<serde_json::Value, String> {
    app.state::<PluginHandle<Wry>>()
        .run_mobile_plugin("inputStatus", serde_json::json!({}))
        .map_err(|e| e.to_string())
}
pub fn pick_macro(
    app: &tauri::AppHandle,
    options: serde_json::Value,
) -> Result<serde_json::Value, String> {
    app.state::<PluginHandle<Wry>>()
        .run_mobile_plugin("pickMacro", options)
        .map_err(|e| e.to_string())
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
    runtime().active().into()
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
extern "system" fn Java_com_acrux_next_Native_agentInputCurrent(
    _: JNIEnv,
    _: JClass,
    generation: jint,
    id: jint,
) -> jboolean {
    let state = runtime();
    (state.input_current(generation) && state.phone.can_start(generation, id, crate::now())).into()
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_agentLaunchStart(
    _: JNIEnv,
    _: JClass,
    generation: jint,
    id: jint,
    unlocked: jboolean,
) -> jboolean {
    crate::phone::claim_launch(&mut runtime(), generation, id, unlocked != 0, crate::now()).into()
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_overlayPhase(_: JNIEnv, _: JClass) -> jint {
    let state = runtime();
    match state.phase() {
        crate::Phase::Idle if state.active() => 2,
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
    let message = runtime().message().to_owned();
    env.new_string(message)
        .map_or(std::ptr::null_mut(), |s| s.into_raw())
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_noTarget(_: JNIEnv, _: JClass) {
    let mut state = runtime();
    if state.dua.running {
        crate::phone::foreground(&mut state, false, crate::now());
    } else {
        state.no_target();
    }
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
extern "system" fn Java_com_acrux_next_Native_macroAction(
    mut env: JNIEnv,
    _: JClass,
    action: JString,
    payload: JString,
) -> jstring {
    let result = (|| {
        let action: String = env.get_string(&action).map_err(|e| e.to_string())?.into();
        let payload: String = env.get_string(&payload).map_err(|e| e.to_string())?.into();
        if payload.len() > 1024 * 1024 {
            return Err("작업 요청이 너무 큽니다.".into());
        }
        let payload: serde_json::Value =
            serde_json::from_str(&payload).map_err(|e| e.to_string())?;
        let generation = payload["_generation"]
            .as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .ok_or("작업의 실행 세대가 없습니다.")?;
        crate::automation_call(action, payload, generation).map(|_| ())
    })();
    reply(&env, result)
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
        .and_then(crate::task_call);
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
    let result = if state.active() {
        Err("실행 중인 작업을 먼저 중지하세요.".into())
    } else {
        let previous = state.macro_settings.clone();
        let mut config = state.macro_config();
        let feat = if [
            "sell_fish_pos",
            "first_fish_pos",
            "sell_all_pos",
            "confirm_sell_pos",
            "shop_close_pos",
        ]
        .contains(&key.as_str())
        {
            "mfish"
        } else {
            "base"
        };
        config[feat][&key] = serde_json::json!([x, y]);
        state.macro_settings = config;
        let result = state.edit_navigation(|nav| nav.set_point(&key, Point(x, y)));
        if result.is_err() {
            state.macro_settings = previous;
        }
        result
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

fn agent_array(env: &JNIEnv, values: Vec<i32>) -> jintArray {
    let reserved =
        (values.first() == Some(&2) && values.len() >= 9).then(|| (values[8], values[1]));
    let result = env.new_int_array(values.len() as i32).and_then(|array| {
        env.set_int_array_region(&array, 0, &values)?;
        Ok(array.into_raw())
    });
    match result {
        Ok(array) => array,
        Err(_) => {
            if let Some((generation, input_id)) = reserved {
                crate::phone::ack(&mut runtime(), generation, input_id, false, crate::now());
            }
            std::ptr::null_mut()
        }
    }
}

fn invalid_observation(generation: i32, observation_id: i32) -> jintArray {
    crate::phone::reject_observation(&mut runtime(), generation, observation_id);
    std::ptr::null_mut()
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_agentTick(
    env: JNIEnv,
    _: JClass,
    width: jint,
    height: jint,
) -> jintArray {
    let values = crate::phone::tick(
        &mut runtime(),
        width.max(0) as usize,
        height.max(0) as usize,
        crate::now(),
    );
    agent_array(&env, values)
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_agentObserve(
    env: JNIEnv,
    _: JClass,
    buffer: JByteBuffer,
    width: jint,
    height: jint,
    row_stride: jint,
    pixel_stride: jint,
    x: jint,
    y: jint,
    crop_width: jint,
    crop_height: jint,
    generation: jint,
    observation_id: jint,
    age_ms: jint,
) -> jintArray {
    if width <= 0
        || height <= 0
        || width > 16384
        || height > 16384
        || row_stride <= 0
        || pixel_stride < 3
        || x < 0
        || y < 0
        || crop_width <= 0
        || crop_height <= 0
    {
        return invalid_observation(generation, observation_id);
    }
    let Ok(pointer) = env.get_direct_buffer_address(&buffer) else {
        return invalid_observation(generation, observation_id);
    };
    let Ok(capacity) = env.get_direct_buffer_capacity(&buffer) else {
        return invalid_observation(generation, observation_id);
    };
    if capacity > isize::MAX as usize {
        return invalid_observation(generation, observation_id);
    }
    // Kotlin keeps the Image or packed ROI buffer alive throughout this call.
    // bytes[0] is the ROI origin; cropped rows use their own packed row stride.
    let bytes = unsafe { std::slice::from_raw_parts(pointer, capacity) };
    let region = crate::vision::Region {
        x: x as usize,
        y: y as usize,
        width: crop_width as usize,
        height: crop_height as usize,
    };
    let Some(frame) = Frame::region(
        bytes,
        width as usize,
        height as usize,
        row_stride as usize,
        pixel_stride as usize,
        false,
        region,
    ) else {
        return invalid_observation(generation, observation_id);
    };
    let result = crate::phone::observe_with_age(
        &mut runtime(),
        &frame,
        generation,
        observation_id,
        crate::now(),
        f64::from(age_ms) / 1000.,
    );
    result.map_or(std::ptr::null_mut(), |values| agent_array(&env, values))
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_agentText(
    mut env: JNIEnv,
    _: JClass,
    generation: jint,
    observation_id: jint,
    words: JString,
) -> jboolean {
    let Ok(json) = env.get_string(&words) else {
        return false.into();
    };
    let json = String::from(json);
    if json.len() > 1024 * 1024 {
        return false.into();
    }
    match serde_json::from_str::<Vec<crate::automation::Word>>(&json) {
        Ok(words) => crate::phone::text(&mut runtime(), generation, observation_id, words).into(),
        Err(_) => false.into(),
    }
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_agentForeground(
    _: JNIEnv,
    _: JClass,
    visible: jboolean,
) {
    crate::phone::foreground(&mut runtime(), visible != 0, crate::now());
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_agentInputResult(
    _: JNIEnv,
    _: JClass,
    generation: jint,
    input_id: jint,
    success: jboolean,
) {
    crate::phone::ack(
        &mut runtime(),
        generation,
        input_id,
        success != 0,
        crate::now(),
    );
}

#[no_mangle]
extern "system" fn Java_com_acrux_next_Native_navigationAction(
    mut env: JNIEnv,
    _: JClass,
    action: JString,
    width: jint,
    height: jint,
) -> jstring {
    let result = match env.get_string(&action) {
        Ok(action) if width > 0 && height > 0 => {
            runtime().device_command(&String::from(action), (width as usize, height as usize))
        }
        _ => Err("기기 동작과 화면 크기를 확인하세요.".into()),
    };
    reply(&env, result)
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
