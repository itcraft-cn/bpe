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
        Box::new(PythonFfiFunc {
            _callback: callback,
        }),
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

#[derive(Debug)]
struct PythonFfiFunc {
    _callback: PyObject,
}
impl FfiFunc for PythonFfiFunc {
    fn callback(&self, _data: Vec<[u64; 64]>) {}
}
