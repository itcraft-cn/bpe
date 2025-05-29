use std::slice;

use bambootube::{Column, FfiFunc, U8Bytes};
use pyo3::prelude::*;

#[pyfunction]
#[allow(dead_code)]
fn start() -> PyResult<()> {
    bambootube::start();
    Ok(())
}

#[pyfunction]
#[allow(dead_code)]
fn stop() -> PyResult<()> {
    bambootube::stop();
    Ok(())
}

#[pyfunction]
#[allow(dead_code)]
fn def_incoming(
    name: &str,
    field_names: Vec<&str>,
    field_types: Vec<u16>,
    field_sizes: Vec<usize>,
) -> PyResult<i32> {
    def_record(
        name,
        field_names,
        field_types,
        field_sizes,
        bambootube::def_incoming,
    )
}

#[pyfunction]
#[allow(dead_code)]
fn def_stream(
    name: &str,
    field_names: Vec<&str>,
    field_types: Vec<u16>,
    field_sizes: Vec<usize>,
) -> PyResult<i32> {
    def_record(
        name,
        field_names,
        field_types,
        field_sizes,
        bambootube::def_stream,
    )
}

fn def_record<F>(
    name: &str,
    field_names: Vec<&str>,
    field_types: Vec<u16>,
    field_sizes: Vec<usize>,
    f: F,
) -> PyResult<i32>
where
    F: Fn(&str, Vec<Column>) -> Option<u16>,
{
    let names_len = field_names.len();
    let types_len = field_types.len();
    let sizes_len = field_sizes.len();
    if names_len != types_len || names_len != sizes_len {
        return Ok(-1);
    }
    let mut columns = vec![];
    for i in 0..names_len {
        columns.push(Column::new(
            String::from(field_names[i]),
            field_types[i],
            field_sizes[i],
        ));
    }
    if let Some(id) = f(name, columns) {
        Ok(id as i32)
    } else {
        Ok(-1)
    }
}

#[pyfunction]
#[allow(dead_code)]
fn new_data(id: u16, bdata: &[u8]) -> PyResult<bool> {
    let data = U8Bytes::new_from_slice(id, bdata.len(), bdata);
    Ok(bambootube::new_data(&data))
}

#[pyfunction]
#[allow(dead_code)]
fn def_mapper(sql: &str, callback: PyObject) -> PyResult<i32> {
    def_action(sql, callback, bambootube::def_mapper_ffi)
}

#[pyfunction]
#[allow(dead_code)]
fn def_mapper_bind_aggregate(sql: &str, aggregate_id: u16) -> PyResult<i32> {
    if let Some(id) = bambootube::def_mapper_bind_aggregate(sql, aggregate_id) {
        Ok(id as i32)
    } else {
        Ok(-1)
    }
}

#[pyfunction]
#[allow(dead_code)]
fn def_aggregate(sql: &str, callback: PyObject) -> PyResult<i32> {
    def_action(sql, callback, bambootube::def_aggregate_ffi)
}

fn def_action<F>(sql: &str, callback: PyObject, f: F) -> PyResult<i32>
where
    F: Fn(&str, Box<dyn FfiFunc>) -> Option<u16>,
{
    if let Some(id) = f(sql, Box::new(PythonFfiFunc { callback })) {
        Ok(id as i32)
    } else {
        Ok(-1)
    }
}

/// A Python module implemented in Rust.
#[pymodule]
#[allow(dead_code)]
fn bambootube4py(_py: Python, m: &PyModule) -> PyResult<()> {
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
    fn callback(&self, data_ptr: *const u8, size: usize) {
        Python::with_gil(|py| {
            let data = unsafe {
                let array_ptr = data_ptr as *const [u8; 512];
                slice::from_raw_parts(array_ptr, size)
            };
            call_py_func(py, &self.callback, data);
        })
    }
}

fn call_py_func(py: Python, callback: &PyObject, data: &[[u8; 512]]) {
    if let Ok(func) = callback.getattr(py, "callback") {
        let array = conv_array(data);
        let args = (array,);
        let rs = func.call1(py, args);
        if rs.is_err() {
            log::warn!("call python method failed: {:?}", rs.err().unwrap());
        }
    }
}

fn conv_array(data: &[[u8; 512]]) -> Vec<u8> {
    let len = data.len();
    let mut vec = vec![0_u8; len * 512];
    for i in 0..len {
        vec[i * 512..(i + 1) * 512].copy_from_slice(&data[i]);
    }
    vec
}
