use pyo3::prelude::*;
use shower::{Column, FfiFunc, U8Bytes};

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
fn def_incoming(
    name: &str,
    field_names: Vec<&str>,
    field_types: Vec<u16>,
    field_sizes: Vec<usize>,
) -> PyResult<i32> {
    let names_len = field_names.len();
    let types_len = field_types.len();
    let sizes_len = field_sizes.len();
    if names_len != types_len || names_len != sizes_len {
        return Ok(-1);
    }
    let mut columns = vec![];
    for i in 0..names_len {
        columns.push(Column::new(field_names[i], field_types[i], field_sizes[i]));
    }
    if let Some(id) = shower::def_incoming(name, columns) {
        Ok(id as i32)
    } else {
        Ok(-1)
    }
}

#[pyfunction]
#[allow(dead_code)]
fn def_stream(
    name: &str,
    field_names: Vec<&str>,
    field_types: Vec<u16>,
    field_sizes: Vec<usize>,
) -> PyResult<i32> {
    let names_len = field_names.len();
    let types_len = field_types.len();
    let sizes_len = field_sizes.len();
    if names_len != types_len || names_len != sizes_len {
        return Ok(-1);
    }
    let mut columns = vec![];
    for i in 0..names_len {
        columns.push(Column::new(field_names[i], field_types[i], field_sizes[i]));
    }
    if let Some(id) = shower::def_incoming(name, columns) {
        Ok(id as i32)
    } else {
        Ok(-1)
    }
}

#[pyfunction]
#[allow(dead_code)]
fn new_data(id: u16, bdata: &[u8]) -> PyResult<bool> {
    let data = U8Bytes::new_from_slice(id, bdata.len(), bdata);
    Ok(shower::new_data(&data))
}

#[pyfunction]
#[allow(dead_code)]
fn def_mapper(sql: &str, callback: PyObject) -> PyResult<i32> {
    if let Some(id) = shower::def_mapper_ffi(sql, Box::new(PythonFfiFunc { callback })) {
        Ok(id as i32)
    } else {
        Ok(-1)
    }
}

#[pyfunction]
#[allow(dead_code)]
fn def_mapper_bind_aggregate(sql: &str, aggregate_id: u16) -> PyResult<i32> {
    if let Some(id) = shower::def_mapper_bind_aggregate(sql, aggregate_id) {
        Ok(id as i32)
    } else {
        Ok(-1)
    }
}

#[pyfunction]
#[allow(dead_code)]
fn def_aggregate(sql: &str, callback: PyObject) -> PyResult<i32> {
    if let Some(id) = shower::def_aggregate_ffi(sql, Box::new(PythonFfiFunc { callback })) {
        Ok(id as i32)
    } else {
        Ok(-1)
    }
}

/// A Python module implemented in Rust.
#[pymodule]
#[allow(dead_code)]
fn shower4py(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(start, m)?)?;
    m.add_function(wrap_pyfunction!(stop, m)?)?;
    m.add_function(wrap_pyfunction!(def_incoming, m)?)?;
    m.add_function(wrap_pyfunction!(def_stream, m)?)?;
    m.add_function(wrap_pyfunction!(new_data, m)?)?;
    m.add_function(wrap_pyfunction!(def_mapper, m)?)?;
    m.add_function(wrap_pyfunction!(def_mapper_bind_aggregate, m)?)?;
    m.add_function(wrap_pyfunction!(def_aggregate, m)?)?;
    Ok(())
}

struct PythonFfiFunc {
    callback: PyObject,
}
impl FfiFunc for PythonFfiFunc {
    fn callback(&self, data: Vec<[u8; 512]>) {
        Python::with_gil(|py| {
            call_py_func(py, &self.callback, data);
        })
    }
}

fn call_py_func(py: Python, callback: &PyObject, data: Vec<[u8; 512]>) {
    if let Ok(func) = callback.getattr(py, "callback") {
        let array = conv_array(data);
        let args = (array,);
        let rs = func.call1(py, args);
        if rs.is_err() {
            log::warn!("call python method failed: {:?}", rs.err().unwrap());
        }
    }
}

fn conv_array(data: Vec<[u8; 512]>) -> Vec<u8> {
    let len = data.len();
    let mut vec = vec![0_u8; len * 512];
    for i in 0..len {
        vec[i * 512..(i + 1) * 512].copy_from_slice(&data[i]);
    }
    vec
}
