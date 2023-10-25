use pyo3::prelude::*;
use shower::{FfiFunc, U8Bytes};

#[pyfunction]
#[allow(dead_code)]
fn start() -> PyResult<bool> {
    Ok(shower::start())
}

#[pyfunction]
#[allow(dead_code)]
fn stop() -> PyResult<bool> {
    shower::stop();
    Ok(true)
}

#[pyfunction]
#[allow(dead_code)]
fn new_data(id: u16, bdata: &[u8]) -> PyResult<bool> {
    let data = U8Bytes::new_from_slice(id, bdata.len(), bdata);
    Ok(shower::new_data(&data))
}

#[pyfunction]
#[allow(dead_code)]
fn def_action(sql: &str) -> PyResult<bool> {
    Ok(shower::def_action(sql))
}

#[pyfunction]
#[allow(dead_code)]
fn def_action_with_callback(sql: &str, callback: PyObject) -> PyResult<bool> {
    Ok(shower::def_action_ffi(
        sql,
        Box::new(PythonFfiFunc { callback }),
    ))
}

/// A Python module implemented in Rust.
#[pymodule]
#[allow(dead_code)]
fn shower4py(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(start, m)?)?;
    m.add_function(wrap_pyfunction!(stop, m)?)?;
    m.add_function(wrap_pyfunction!(new_data, m)?)?;
    m.add_function(wrap_pyfunction!(def_action, m)?)?;
    m.add_function(wrap_pyfunction!(def_action_with_callback, m)?)?;
    Ok(())
}

struct PythonFfiFunc {
    callback: PyObject,
}
impl FfiFunc for PythonFfiFunc {
    fn callback(&self, data: Vec<[u64; 64]>) {
        Python::with_gil(|py| {
            fun_name(py, &self.callback, data);
        })
    }
}

fn fun_name(py: Python, callback: &PyObject, data: Vec<[u64; 64]>) {
    if let Ok(func) = callback.getattr(py, "callback") {
        let array = conv_array(data);
        let args = (array,);
        let rs = func.call1(py, args);
        if rs.is_err() {
            log::warn!("call_method failed: {:?}", rs.err().unwrap());
        }
    }
}

fn conv_array(data: Vec<[u64; 64]>) -> Vec<u64> {
    let mut vec = vec![];
    for array in data {
        vec.push(array[0]);
        vec.push(array[1]);
        vec.push(array[2]);
        vec.push(array[3]);
        vec.push(array[4]);
        vec.push(array[5]);
        vec.push(array[6]);
        vec.push(array[7]);
        vec.push(array[8]);
        vec.push(array[9]);
        vec.push(array[10]);
        vec.push(array[11]);
        vec.push(array[12]);
        vec.push(array[13]);
        vec.push(array[14]);
        vec.push(array[15]);
        vec.push(array[16]);
        vec.push(array[17]);
        vec.push(array[18]);
        vec.push(array[19]);
        vec.push(array[20]);
        vec.push(array[21]);
        vec.push(array[22]);
        vec.push(array[23]);
        vec.push(array[24]);
        vec.push(array[25]);
        vec.push(array[26]);
        vec.push(array[27]);
        vec.push(array[28]);
        vec.push(array[29]);
        vec.push(array[30]);
        vec.push(array[31]);
        vec.push(array[32]);
        vec.push(array[33]);
        vec.push(array[34]);
        vec.push(array[35]);
        vec.push(array[36]);
        vec.push(array[37]);
        vec.push(array[38]);
        vec.push(array[39]);
        vec.push(array[40]);
        vec.push(array[41]);
        vec.push(array[42]);
        vec.push(array[43]);
        vec.push(array[44]);
        vec.push(array[45]);
        vec.push(array[46]);
        vec.push(array[47]);
        vec.push(array[48]);
        vec.push(array[49]);
        vec.push(array[50]);
        vec.push(array[51]);
        vec.push(array[52]);
        vec.push(array[53]);
        vec.push(array[54]);
        vec.push(array[55]);
        vec.push(array[56]);
        vec.push(array[57]);
        vec.push(array[58]);
        vec.push(array[59]);
        vec.push(array[60]);
        vec.push(array[61]);
        vec.push(array[62]);
        vec.push(array[63]);
    }
    vec
}
