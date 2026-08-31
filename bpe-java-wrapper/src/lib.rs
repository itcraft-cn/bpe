#![allow(non_snake_case)]

use bpe::{CallbackParams, Column, FfiFunc, U8Bytes, Window};
use jni::{
    objects::{
        GlobalRef, JByteArray, JClass, JIntArray, JObject, JObjectArray, JPrimitiveArray, JString,
        JValueGen,
    },
    sys::{jboolean, jint, jlong},
    JNIEnv, JavaVM,
};
use std::{slice, sync::Once};

#[no_mangle]
pub extern "system" fn Java_cn_itcraft_bpe4j_Bpe_start<'local>(
    _env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jboolean {
    static START: Once = Once::new();
    START.call_once(bpe::start);
    jboolean::from(true)
}

#[no_mangle]
pub extern "system" fn Java_cn_itcraft_bpe4j_Bpe_stop<'local>(
    _env: JNIEnv<'local>,
    _class: JClass<'local>,
) {
    static STOP: Once = Once::new();
    STOP.call_once(bpe::stop);
}

/// Selects callback delivery mode: 0 = synchronous (legacy), 1 = asynchronous
/// (callbacks run on the engine's egress thread, off the hot path).
/// JavaBpe.start() switches to async automatically.
#[no_mangle]
pub extern "system" fn Java_cn_itcraft_bpe4j_Bpe_setDeliveryMode<'local>(
    _env: JNIEnv<'local>,
    _class: JClass<'local>,
    mode: jint,
) {
    let mode = if mode == 1 {
        bpe::DeliveryMode::Async
    } else {
        bpe::DeliveryMode::Sync
    };
    bpe::set_delivery_mode(mode);
}

/// Number of callback events dropped by the Drain policy (the unbounded
/// egress channel itself never drops events).
#[no_mangle]
pub extern "system" fn Java_cn_itcraft_bpe4j_Bpe_droppedEvents<'local>(
    _env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jlong {
    bpe::dropped_events() as jlong
}

/// Number of events waiting in the egress channel.
#[no_mangle]
pub extern "system" fn Java_cn_itcraft_bpe4j_Bpe_pendingEvents<'local>(
    _env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jlong {
    bpe::pending_events() as jlong
}

/// Total number of timeout alerts raised by the egress watchdog.
#[no_mangle]
pub extern "system" fn Java_cn_itcraft_bpe4j_Bpe_alertCount<'local>(
    _env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jlong {
    bpe::alert_count() as jlong
}

/// Configures the egress watchdog policy (1=Drain, 2=Failover, 3=AlertOnly)
/// and the callback time threshold in milliseconds.
#[no_mangle]
pub extern "system" fn Java_cn_itcraft_bpe4j_Bpe_setEgressPolicy<'local>(
    _env: JNIEnv<'local>,
    _class: JClass<'local>,
    policy: jint,
    threshold_ms: jlong,
) {
    let policy = match policy {
        1 => bpe::EgressPolicy::Drain,
        2 => bpe::EgressPolicy::Failover,
        _ => bpe::EgressPolicy::AlertOnly,
    };
    bpe::set_egress_policy(policy, threshold_ms.max(1) as u64);
}

/// Registers a Java alert listener invoked on egress callback timeouts.
/// The listener receives `(byte[32], 1)` laid out as four little-endian i64:
/// `[elapsed_ms][pending][dropped][policy]`. Passing null clears the listener
/// (alerts fall back to engine logs).
#[no_mangle]
pub extern "system" fn Java_cn_itcraft_bpe4j_Bpe_setAlertListener<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    j_callback: JObject<'local>,
) {
    if j_callback.is_null() {
        bpe::set_alert_listener(None);
        return;
    }
    let rs_vm = env.get_java_vm();
    let rs_ref = env.new_global_ref(j_callback);
    if let (Ok(vm), Ok(callback)) = (rs_vm, rs_ref) {
        bpe::set_alert_listener(Some(Box::new(JavaFfiFunc { vm, callback })));
    } else {
        log::warn!("setAlertListener: failed to capture vm/global ref");
    }
}

#[no_mangle]
pub extern "system" fn Java_cn_itcraft_bpe4j_Bpe_defIncoming<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    j_name: JString<'local>,
    j_names: JObjectArray<'local>,
    j_types: JIntArray<'local>,
    j_sizes: JIntArray<'local>,
) -> jint {
    def_record(
        &mut env,
        j_name,
        j_names,
        j_types,
        j_sizes,
        bpe::def_incoming,
    )
}

#[no_mangle]
pub extern "system" fn Java_cn_itcraft_bpe4j_Bpe_defStream<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    j_name: JString<'local>,
    j_names: JObjectArray<'local>,
    j_types: JIntArray<'local>,
    j_sizes: JIntArray<'local>,
) -> jint {
    def_record(
        &mut env,
        j_name,
        j_names,
        j_types,
        j_sizes,
        bpe::def_stream,
    )
}

fn def_record<'local, F>(
    env: &mut JNIEnv<'local>,
    j_name: JString<'local>,
    j_names: JObjectArray<'local>,
    j_types: JIntArray<'local>,
    j_sizes: JIntArray<'local>,
    f: F,
) -> jint
where
    F: Fn(&str, Vec<Column>) -> Option<u16>,
{
    let name = if let Ok(nm) = conv(env, j_name) {
        nm
    } else {
        return -1;
    };
    let rs = conv_arrays(env, j_names, j_types, j_sizes);
    if let Ok(mixed) = rs {
        let names_len = mixed.names.len();
        let types_len = mixed.types.len();
        let sizes_len = mixed.sizes.len();
        if names_len != types_len || names_len != sizes_len {
            return -1;
        }
        let mut columns = vec![];
        for i in 0..names_len {
            columns.push(Column::new(
                mixed.names[i].clone(),
                mixed.types[i],
                mixed.sizes[i],
            ));
        }
        if let Some(id) = f(&name, columns) {
            id as jint
        } else {
            -1
        }
    } else {
        log::warn!("failed to convert arrays: {:?}", rs.err());
        -1
    }
}

#[no_mangle]
pub extern "system" fn Java_cn_itcraft_bpe4j_Bpe_newData<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    j_record_id: jint,
    j_bdata: JByteArray<'local>,
) -> jboolean {
    let record_id = j_record_id as u16;
    let bytes_vec = env.convert_byte_array(j_bdata).unwrap();
    let data = U8Bytes::new_from_vec(record_id, bytes_vec.len(), bytes_vec);
    bpe::new_data(&data) as jboolean
}

#[no_mangle]
pub extern "system" fn Java_cn_itcraft_bpe4j_Bpe_defMapper<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    j_sql: JString<'local>,
    j_callback: JObject<'local>,
) -> jint {
    def_action(env, j_sql, j_callback, bpe::def_mapper_ffi)
}

#[no_mangle]
pub extern "system" fn Java_cn_itcraft_bpe4j_Bpe_defMapperBindAggregate<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    j_sql: JString<'local>,
    j_aggregate: jint,
) -> jint {
    let rs = conv(&env, j_sql);
    if let Ok(sql) = rs {
        let aggregate_id = j_aggregate as u16;
        if let Some(id) = bpe::def_mapper_bind_aggregate(sql.as_str(), aggregate_id) {
            id as jint
        } else {
            -1
        }
    } else {
        log::warn!("def_mapper_with_callback failed: {:?}", rs.err().unwrap());
        -1
    }
}

#[no_mangle]
/// defWindowAggregate: sql, windowType, periodMs, lengthMs, slideMs, tsField, lagMs, callback
pub extern "system" fn Java_cn_itcraft_bpe4j_Bpe_defWindowAggregate<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    j_sql: JString<'local>,
    j_window_type: jint,
    j_period_ms: jlong,
    j_length_ms: jlong,
    j_slide_ms: jlong,
    j_ts_field: JString<'local>,
    j_lag_ms: jlong,
    j_callback: JObject<'local>,
) -> jint {
    def_window_action(env, j_sql, j_window_type, j_period_ms, j_length_ms, j_slide_ms, j_ts_field, None, j_lag_ms, j_callback, |sql, window, ts, _key, lag, ffi| {
        bpe::def_window_aggregate_ffi(sql, window, ts, lag, ffi)
    })
}

/// defKeyedWindowAggregate: sql, windowType, periodMs, lengthMs, slideMs, tsField, keyField, lagMs, callback
#[no_mangle]
pub extern "system" fn Java_cn_itcraft_bpe4j_Bpe_defKeyedWindowAggregate<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    j_sql: JString<'local>,
    j_window_type: jint,
    j_period_ms: jlong,
    j_length_ms: jlong,
    j_slide_ms: jlong,
    j_ts_field: JString<'local>,
    j_key_field: JString<'local>,
    j_lag_ms: jlong,
    j_callback: JObject<'local>,
) -> jint {
    def_window_action(env, j_sql, j_window_type, j_period_ms, j_length_ms, j_slide_ms, j_ts_field, Some(j_key_field), j_lag_ms, j_callback, bpe::def_keyed_window_aggregate_ffi)
}

#[allow(clippy::too_many_arguments)]
fn def_window_action<'local, F>(
    env: JNIEnv<'local>,
    j_sql: JString<'local>,
    j_window_type: jint,
    j_period_ms: jlong,
    j_length_ms: jlong,
    j_slide_ms: jlong,
    j_ts_field: JString<'local>,
    j_key_field: Option<JString<'local>>,
    j_lag_ms: jlong,
    j_callback: JObject<'local>,
    f: F,
) -> jint
where
    F: Fn(&str, Window, Option<&str>, Option<&str>, u64, Box<dyn FfiFunc>) -> Option<u16>,
{
    let window = match j_window_type {
        1 => Window::Tumbling {
            period_ms: j_period_ms as u64,
        },
        2 => Window::Sliding {
            length_ms: j_length_ms as u64,
            slide_ms: j_slide_ms as u64,
        },
        _ => {
            log::warn!("unknown window_type: {j_window_type}");
            return -1;
        }
    };
    let sql = match conv(&env, j_sql) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("conv sql failed: {e:?}");
            return -1;
        }
    };
    let ts_field = if j_ts_field.is_null() {
        None
    } else {
        match conv(&env, j_ts_field) {
            Ok(s) => Some(s),
            Err(_) => return -1,
        }
    };
    let key_field = match j_key_field {
        Some(jk) => {
            if jk.is_null() {
                None
            } else {
                match conv(&env, jk) {
                    Ok(s) => Some(s),
                    Err(_) => return -1,
                }
            }
        }
        None => None,
    };
    let vm = match env.get_java_vm() {
        Ok(vm) => vm,
        Err(e) => {
            log::warn!("failed to get java vm: {e:?}");
            return -1;
        }
    };
    let callback = match env.new_global_ref(j_callback) {
        Ok(cb) => cb,
        Err(e) => {
            log::warn!("failed to create global ref: {e:?}");
            return -1;
        }
    };
    let ffi = Box::new(JavaFfiFunc { vm, callback });
    match f(
        sql.as_str(),
        window,
        ts_field.as_deref(),
        key_field.as_deref(),
        j_lag_ms as u64,
        ffi,
    ) {
        Some(id) => id as jint,
        None => -1,
    }
}

/// defDimension: allocates a dimension table
#[no_mangle]
pub extern "system" fn Java_cn_itcraft_bpe4j_Bpe_defDimension<'local>(
    _env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jint {
    bpe::def_dimension().map(|id| id as jint).unwrap_or(-1)
}

/// updateDimension(id, key, value)
#[no_mangle]
pub extern "system" fn Java_cn_itcraft_bpe4j_Bpe_updateDimension<'local>(
    _env: JNIEnv<'local>,
    _class: JClass<'local>,
    j_id: jint,
    j_key: jlong,
    j_value: jlong,
) {
    bpe::update_dimension(j_id as u16, j_key, j_value);
}

/// removeDimension(id, key)
#[no_mangle]
pub extern "system" fn Java_cn_itcraft_bpe4j_Bpe_removeDimension<'local>(
    _env: JNIEnv<'local>,
    _class: JClass<'local>,
    j_id: jint,
    j_key: jlong,
) {
    bpe::remove_dimension(j_id as u16, j_key);
}

#[no_mangle]
pub extern "system" fn Java_cn_itcraft_bpe4j_Bpe_defAggregate<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    j_sql: JString<'local>,
    j_callback: JObject<'local>,
) -> jint {
    def_action(env, j_sql, j_callback, bpe::def_aggregate_ffi)
}

fn def_action<'local, F>(
    env: JNIEnv<'local>,
    j_sql: JString<'local>,
    j_callback: JObject<'local>,
    f: F,
) -> jint
where
    F: Fn(&str, Box<dyn FfiFunc>) -> Option<u16>,
{
    let rs = conv(&env, j_sql);
    if let Ok(sql) = rs {
        let rs = env.get_java_vm();
        if let Ok(vm) = rs {
            let rs = env.new_global_ref(j_callback);
            if let Ok(callback) = rs {
                if let Some(id) = def_java_callback(f, sql, callback, vm) {
                    id as jint
                } else {
                    -1
                }
            } else {
                log::warn!(
                    "failed to create global reference to callback object: {:?}",
                    rs.err().unwrap()
                );
                -1
            }
        } else {
            log::warn!("failed to get java vm: {:?}", rs.err().unwrap());
            -1
        }
    } else {
        log::warn!("def_mapper_with_callback failed: {:?}", rs.err().unwrap());
        -1
    }
}

fn def_java_callback<F>(f: F, sql: String, callback: GlobalRef, vm: JavaVM) -> Option<u16>
where
    F: Fn(&str, Box<dyn FfiFunc>) -> Option<u16>,
{
    f(sql.as_str(), Box::new(JavaFfiFunc { vm, callback }))
}

fn conv(env: &JNIEnv, java_str: JString) -> Result<String, String> {
    if let Ok(rust_str) = unsafe { env.get_string_unchecked(&java_str) } {
        Ok(rust_str.into())
    } else {
        Err("Could not convert java string to rust string".to_string())
    }
}

#[derive(Debug)]
struct JavaFfiFunc {
    vm: JavaVM,
    callback: GlobalRef,
}
impl FfiFunc for JavaFfiFunc {
    fn callback(&self, params: CallbackParams) {
        let data_ptr = params.u8_ptr();
        let size = params.size();
        // payload real length is size * step (mapper: 512-byte records, window/
        // aggregate results: compact field rows) - never assume a fixed stride
        let len = size.saturating_mul(params.step());
        // In async delivery mode this runs on the engine's egress thread, which
        // has never been attached to the JVM: attach it permanently on first use
        // (repeated attach/detach per event would cost ~us each).
        let rs_env = self
            .vm
            .get_env()
            .or_else(|_| self.vm.attach_current_thread_permanently());
        if let Ok(mut env) = rs_env {
            let data = unsafe { slice::from_raw_parts(data_ptr, len) };
            let i8slice =
                unsafe { &*(data as *const [u8] as *const [i8]) };
            let array = env.new_byte_array(len as i32).unwrap();
            let _ = env.set_byte_array_region(&array, 0, i8slice);
            let param1 = JValueGen::Object(&array as &JObject);
            let param2 = JValueGen::Int(size as i32);
            let rs = env.call_method(
                self.callback.clone(),
                "callback",
                "([BI)V",
                &[param1, param2],
            );
            if rs.is_err() {
                log::warn!("call_method failed: {:?}", rs.err().unwrap());
            }
        }
    }
}


fn conv_arrays<'a>(
    env: &mut JNIEnv<'a>,
    j_names: JObjectArray<'a>,
    j_types: JIntArray<'a>,
    j_sizes: JIntArray<'a>,
) -> Result<MixedVec, String> {
    let rs1 = conv_string_array(env, j_names);
    let array1 = if let Ok(array) = rs1 {
        array
    } else {
        return Err(String::from(
            "failed to conver java string array to rust vec",
        ));
    };
    let rs2 = conv_int_array(env, j_types, |v| v as u16);
    let array2 = if let Ok(array) = rs2 {
        array
    } else {
        return Err(String::from(
            "failed to conver java string array to rust vec",
        ));
    };
    let rs3 = conv_int_array(env, j_sizes, |v| v as usize);
    let array3 = if let Ok(array) = rs3 {
        array
    } else {
        return Err(String::from(
            "failed to conver java string array to rust vec",
        ));
    };
    Ok(MixedVec::new(array1, array2, array3))
}

fn conv_string_array(
    env: &mut JNIEnv<'_>,
    j_str_array: JObjectArray<'_>,
) -> Result<Vec<String>, String> {
    let mut vec = vec![];
    if let Ok(len) = env.get_array_length(&j_str_array) {
        for i in 0..len {
            if let Ok(j_str) = env.get_object_array_element(&j_str_array, i) {
                if let Ok(str) = conv(env, JString::from(j_str)) {
                    vec.push(str);
                } else {
                    return Err(String::from("failed to conv java string to rust string"));
                }
            } else {
                return Err(String::from("failed to get string array elements"));
            }
        }
        Ok(vec)
    } else {
        Err(String::from("failed to get array length"))
    }
}

fn conv_int_array<T>(
    env: &JNIEnv<'_>,
    j_int_array: JPrimitiveArray<'_, i32>,
    f: impl Fn(i32) -> T,
) -> Result<Vec<T>, String> {
    let mut vec = vec![];
    if let Ok(len) = env.get_array_length(&j_int_array) {
        let mut i32vec = vec![0_i32; len as usize];
        let rs = env.get_int_array_region(j_int_array, 0, i32vec.as_mut_slice());
        if rs.is_err() {
            return Err(String::from("failed to get int array region"));
        }
        for i in 0..len {
            vec.push(f(i32vec[i as usize]))
        }
        Ok(vec)
    } else {
        Err(String::from("failed to get array length"))
    }
}

struct MixedVec {
    names: Vec<String>,
    types: Vec<u16>,
    sizes: Vec<usize>,
}
impl MixedVec {
    fn new(names: Vec<String>, types: Vec<u16>, sizes: Vec<usize>) -> MixedVec {
        MixedVec {
            names,
            types,
            sizes,
        }
    }
}
