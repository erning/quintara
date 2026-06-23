//! JNI bridge for the Android app.
//!
//! Keep this crate deliberately small: Kotlin sends JSON DTOs, and the bridge
//! delegates all match behavior to `quintara-mobile`.

#![allow(unsafe_code)] // NOTE: JNI requires stable exported `#[no_mangle]` symbols.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Mutex, OnceLock};

use jni::objects::{JObject, JString};
use jni::sys::{jboolean, jlong, jstring, JNI_FALSE, JNI_TRUE};
use jni::JNIEnv;
use quintara_mobile::MobileSession;
use quintara_rapfi::RapfiMoveSource;

static NEXT_HANDLE: AtomicI64 = AtomicI64::new(1);
static SESSIONS: OnceLock<Mutex<HashMap<i64, MobileSession>>> = OnceLock::new();

#[no_mangle]
pub extern "system" fn Java_com_erning_quintara_engine_NativeEngine_nativeRapfiAvailable(
    _env: JNIEnv<'_>,
    _this: JObject<'_>,
) -> jboolean {
    if RapfiMoveSource::is_available() {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

#[no_mangle]
pub extern "system" fn Java_com_erning_quintara_engine_NativeEngine_nativeCreateSession(
    mut env: JNIEnv<'_>,
    _this: JObject<'_>,
    config_json: JString<'_>,
) -> jlong {
    match create_session(&mut env, &config_json) {
        Ok(handle) => handle,
        Err(message) => {
            throw(&mut env, "java/lang/IllegalArgumentException", &message);
            0
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_erning_quintara_engine_NativeEngine_nativeTick(
    mut env: JNIEnv<'_>,
    _this: JObject<'_>,
    handle: jlong,
    input_json: JObject<'_>,
) -> jstring {
    match tick_session(&mut env, handle, input_json) {
        Ok(json) => new_string(&mut env, &json),
        Err(message) => {
            throw(&mut env, "java/lang/IllegalStateException", &message);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_erning_quintara_engine_NativeEngine_nativeSnapshot(
    mut env: JNIEnv<'_>,
    _this: JObject<'_>,
    handle: jlong,
) -> jstring {
    match snapshot_session(handle) {
        Ok(json) => new_string(&mut env, &json),
        Err(message) => {
            throw(&mut env, "java/lang/IllegalStateException", &message);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_erning_quintara_engine_NativeEngine_nativeExportPsq(
    mut env: JNIEnv<'_>,
    _this: JObject<'_>,
    handle: jlong,
) -> jstring {
    match with_session(handle, |session| session.export_psq()) {
        Ok(psq) => new_string(&mut env, &psq),
        Err(message) => {
            throw(&mut env, "java/lang/IllegalStateException", &message);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_erning_quintara_engine_NativeEngine_nativeDisposeSession(
    mut env: JNIEnv<'_>,
    _this: JObject<'_>,
    handle: jlong,
) {
    if let Err(message) = dispose_session(handle) {
        throw(&mut env, "java/lang/IllegalStateException", &message);
    }
}

fn create_session(env: &mut JNIEnv<'_>, config_json: &JString<'_>) -> Result<jlong, String> {
    let config = java_string(env, config_json)?;
    let mut session = MobileSession::from_json(&config).map_err(|e| e.to_string())?;
    let _ = session.tick(None);
    let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
    sessions()
        .lock()
        .map_err(|_| "mobile session registry is poisoned".to_string())?
        .insert(handle, session);
    Ok(handle)
}

fn tick_session(
    env: &mut JNIEnv<'_>,
    handle: jlong,
    input_json: JObject<'_>,
) -> Result<String, String> {
    let input = nullable_java_string(env, input_json)?;
    with_session(handle, |session| {
        session
            .tick_json(input.as_deref())
            .map_err(|e| e.to_string())
    })?
}

fn snapshot_session(handle: jlong) -> Result<String, String> {
    with_session(handle, |session| {
        serde_json::to_string(&session.snapshot()).map_err(|e| e.to_string())
    })?
}

fn dispose_session(handle: jlong) -> Result<(), String> {
    if handle <= 0 {
        return Ok(());
    }
    sessions()
        .lock()
        .map_err(|_| "mobile session registry is poisoned".to_string())?
        .remove(&handle);
    Ok(())
}

fn with_session<T>(
    handle: jlong,
    action: impl FnOnce(&mut MobileSession) -> T,
) -> Result<T, String> {
    let mut guard = sessions()
        .lock()
        .map_err(|_| "mobile session registry is poisoned".to_string())?;
    let session = guard
        .get_mut(&handle)
        .ok_or_else(|| format!("unknown mobile session handle: {handle}"))?;
    Ok(action(session))
}

fn sessions() -> &'static Mutex<HashMap<i64, MobileSession>> {
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn java_string(env: &mut JNIEnv<'_>, value: &JString<'_>) -> Result<String, String> {
    env.get_string(value)
        .map(Into::into)
        .map_err(|e| e.to_string())
}

fn nullable_java_string(
    env: &mut JNIEnv<'_>,
    value: JObject<'_>,
) -> Result<Option<String>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let string = JString::from(value);
    java_string(env, &string).map(Some)
}

fn new_string(env: &mut JNIEnv<'_>, value: &str) -> jstring {
    env.new_string(value)
        .map_or_else(|_| std::ptr::null_mut(), JString::into_raw)
}

fn throw(env: &mut JNIEnv<'_>, class: &str, message: &str) {
    let _ = env.throw_new(class, message);
}
