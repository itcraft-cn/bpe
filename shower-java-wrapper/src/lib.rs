use jni::{
    objects::{GlobalRef, JByteArray, JClass, JObject, JPrimitiveArray, JString, JValueGen},
    sys::{jboolean, jint},
    JNIEnv, JavaVM,
};
use shower::{def_action, def_action_ffi, new_data, start, stop, FfiFunc, U8Bytes};
use std::sync::Once;

static mut OPT_GLOBAL_REF: Option<Vec<Box<GlobalRef>>> = None;

#[no_mangle]
pub extern "system" fn Java_com_erayt_shower4j_Shower_start<'local>(
    _env: JNIEnv<'local>,
    // This is the class that owns our static method. It's not going to be used,
    // but still must be present to match the expected signature of a static
    // native method.
    _class: JClass<'local>,
) -> jboolean {
    static START: Once = Once::new();
    START.call_once(|| unsafe {
        OPT_GLOBAL_REF.replace(vec![]);
    });
    start() as jboolean
}

#[no_mangle]
pub extern "system" fn Java_com_erayt_shower4j_Shower_stop<'local>(
    _env: JNIEnv<'local>,
    // This is the class that owns our static method. It's not going to be used,
    // but still must be present to match the expected signature of a static
    // native method.
    _class: JClass<'local>,
) {
    stop()
}

#[no_mangle]
pub extern "system" fn Java_com_erayt_shower4j_Shower_newData<'local>(
    env: JNIEnv<'local>,
    // This is the class that owns our static method. It's not going to be used,
    // but still must be present to match the expected signature of a static
    // native method.
    _class: JClass<'local>,
    j_tab_id: jint,
    j_bdata: JByteArray<'local>,
) -> jboolean {
    let tab_id = j_tab_id as u16;
    let bytes_vec = env.convert_byte_array(j_bdata).unwrap();
    let data = U8Bytes::new_from_vec(tab_id, bytes_vec.len(), bytes_vec);
    new_data(&data) as jboolean
}

#[no_mangle]
pub extern "system" fn Java_com_erayt_shower4j_Shower_defAction<'local>(
    env: JNIEnv<'local>,
    // This is the class that owns our static method. It's not going to be used,
    // but still must be present to match the expected signature of a static
    // native method.
    _class: JClass<'local>,
    j_sql: JString<'local>,
) -> jboolean {
    let rs = conv(&env, j_sql);
    if let Ok(sql) = rs {
        def_action(&sql) as jboolean
    } else {
        log::warn!("def_action failed: {:?}", rs.err().unwrap());
        false as jboolean
    }
}

#[no_mangle]
pub extern "system" fn Java_com_erayt_shower4j_Shower_defActionWithCallback<'local>(
    env: JNIEnv<'local>,
    // This is the class that owns our static method. It's not going to be used,
    // but still must be present to match the expected signature of a static
    // native method.
    _class: JClass<'local>,
    j_sql: JString<'local>,
    j_callback: JObject<'local>,
) -> jboolean {
    let rs = conv(&env, j_sql);
    if let Ok(sql) = rs {
        let rs = env.get_java_vm();
        if let Ok(vm) = rs {
            let rs = env.new_global_ref(j_callback);
            if let Ok(callback) = rs {
                def_java_callback(sql, callback, vm) as jboolean
            } else {
                log::warn!(
                    "failed to create global reference to callback object: {:?}",
                    rs.err().unwrap()
                );
                false as jboolean
            }
        } else {
            log::warn!("failed to get java vm: {:?}", rs.err().unwrap());
            false as jboolean
        }
    } else {
        log::warn!("def_action_with_callback failed: {:?}", rs.err().unwrap());
        false as jboolean
    }
}

fn def_java_callback(sql: String, callback: GlobalRef, vm: JavaVM) -> bool {
    def_action_ffi(sql.as_str(), Box::new(JavaFfiFunc { vm, callback }))
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
    fn callback(&self, data: Vec<[u8; 512]>) {
        let rs = self.vm.get_env();
        if let Ok(mut env) = rs {
            let len = data.len() as i32;
            let array = conv_array(&env, data);
            let param1 = JValueGen::Object(&array as &JObject);
            let param2 = JValueGen::Int(len);
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

fn conv_array<'a>(env: &JNIEnv<'a>, data: Vec<[u8; 512]>) -> JPrimitiveArray<'a, i8> {
    let len = data.len();
    let array = env.new_byte_array((len * 512) as i32).unwrap();
    for i in 0..len {
        let u8slice = data[i].as_slice();
        let i8slice = unsafe { &*(u8slice as *const [u8] as *const [i8]) };
        let _ = env.set_byte_array_region(&array, (i * 512) as i32, i8slice);
    }
    array
}
