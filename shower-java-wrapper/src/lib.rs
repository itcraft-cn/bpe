use jni::{
    objects::{
        GlobalRef, JByteArray, JClass, JIntArray, JObject, JObjectArray, JPrimitiveArray, JString,
        JValueGen,
    },
    sys::{jboolean, jint},
    JNIEnv, JavaVM,
};
use shower::{Column, FfiFunc, U8Bytes};
use std::sync::Once;

static mut OPT_GLOBAL_REF: Option<Vec<GlobalRef>> = None;

#[no_mangle]
pub extern "system" fn Java_com_erayt_shower4j_Shower_start<'local>(
    _env: JNIEnv<'local>,
    // This is the class that owns our static method. It's not going to be used,
    // but still must be present to match the expected signature of a static
    // native method.
    _class: JClass<'local>,
) {
    static START: Once = Once::new();
    START.call_once(|| unsafe {
        OPT_GLOBAL_REF.replace(vec![]);
    });
    shower::start();
}

#[no_mangle]
pub extern "system" fn Java_com_erayt_shower4j_Shower_stop<'local>(
    _env: JNIEnv<'local>,
    // This is the class that owns our static method. It's not going to be used,
    // but still must be present to match the expected signature of a static
    // native method.
    _class: JClass<'local>,
) {
    shower::stop()
}

#[no_mangle]
pub extern "system" fn Java_com_erayt_shower4j_Shower_defIncoming<'local>(
    mut env: JNIEnv<'local>,
    // This is the class that owns our static method. It's not going to be used,
    // but still must be present to match the expected signature of a static
    // native method.
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
        shower::def_incoming,
    )
}

#[no_mangle]
pub extern "system" fn Java_com_erayt_shower4j_Shower_defStream<'local>(
    mut env: JNIEnv<'local>,
    // This is the class that owns our static method. It's not going to be used,
    // but still must be present to match the expected signature of a static
    // native method.
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
        shower::def_stream,
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
pub extern "system" fn Java_com_erayt_shower4j_Shower_newData<'local>(
    env: JNIEnv<'local>,
    // This is the class that owns our static method. It's not going to be used,
    // but still must be present to match the expected signature of a static
    // native method.
    _class: JClass<'local>,
    j_record_id: jint,
    j_bdata: JByteArray<'local>,
) -> jboolean {
    let record_id = j_record_id as u16;
    let bytes_vec = env.convert_byte_array(j_bdata).unwrap();
    let data = U8Bytes::new_from_vec(record_id, bytes_vec.len(), bytes_vec);
    shower::new_data(&data) as jboolean
}

#[no_mangle]
pub extern "system" fn Java_com_erayt_shower4j_Shower_defMapper<'local>(
    env: JNIEnv<'local>,
    // This is the class that owns our static method. It's not going to be used,
    // but still must be present to match the expected signature of a static
    // native method.
    _class: JClass<'local>,
    j_sql: JString<'local>,
    j_callback: JObject<'local>,
) -> jint {
    def_action(env, j_sql, j_callback, shower::def_mapper_ffi)
}

#[no_mangle]
pub extern "system" fn Java_com_erayt_shower4j_Shower_defMapperBindAggregate<'local>(
    env: JNIEnv<'local>,
    // This is the class that owns our static method. It's not going to be used,
    // but still must be present to match the expected signature of a static
    // native method.
    _class: JClass<'local>,
    j_sql: JString<'local>,
    j_aggregate: jint,
) -> jint {
    let rs = conv(&env, j_sql);
    if let Ok(sql) = rs {
        let aggregate_id = j_aggregate as u16;
        if let Some(id) = shower::def_mapper_bind_aggregate(sql.as_str(), aggregate_id) {
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
pub extern "system" fn Java_com_erayt_shower4j_Shower_defAggregate<'local>(
    env: JNIEnv<'local>,
    // This is the class that owns our static method. It's not going to be used,
    // but still must be present to match the expected signature of a static
    // native method.
    _class: JClass<'local>,
    j_sql: JString<'local>,
    j_callback: JObject<'local>,
) -> jint {
    def_action(env, j_sql, j_callback, shower::def_aggregate_ffi)
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
    fn callback(&self, data_ptr: *const u8, size: usize) {
        /*
        let rs = self.vm.get_env();
        if let Ok(mut env) = rs {
            let array = conv_array(&env, data, size);
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
         */
    }
}

fn conv_array<'a>(env: &JNIEnv<'a>, data: &[[u8; 512]]) -> JPrimitiveArray<'a, i8> {
    let len = data.len();
    let array = env.new_byte_array((len * 512) as i32).unwrap();
    for (i, item) in data.iter().enumerate().take(len) {
        let u8slice = item.as_slice();
        let i8slice = unsafe { &*(u8slice as *const [u8] as *const [i8]) };
        let _ = env.set_byte_array_region(&array, (i * 512) as i32, i8slice);
    }
    array
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
