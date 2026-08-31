use bpe::{CallbackParams, Column, DeliveryMode, FfiFunc, U8Bytes, Window};
use pyo3::prelude::*;
use std::slice;

const U8_DATA_MAX_SIZE: usize = 512;

#[pyfunction]
#[allow(dead_code)]
fn start() -> PyResult<()> {
    bpe::start();
    Ok(())
}

#[pyfunction]
#[allow(dead_code)]
fn stop() -> PyResult<()> {
    bpe::stop();
    Ok(())
}

/// Selects callback delivery: `is_async=True` delivers callbacks on the
/// engine's egress thread (results are copied into an egress ring, so ingest
/// is never blocked by Python callback execution / the GIL).
#[pyfunction]
#[allow(dead_code)]
fn set_delivery_mode(is_async: bool) -> PyResult<()> {
    let mode = if is_async {
        DeliveryMode::Async
    } else {
        DeliveryMode::Sync
    };
    bpe::set_delivery_mode(mode);
    Ok(())
}

/// Number of callback events dropped because the egress ring was full
/// (async mode only).
#[pyfunction]
#[allow(dead_code)]
fn dropped_events() -> PyResult<u64> {
    Ok(bpe::dropped_events())
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
        bpe::def_incoming,
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
        bpe::def_stream,
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
    Ok(bpe::new_data(&data))
}

#[pyfunction]
#[allow(dead_code)]
fn def_mapper(sql: &str, callback: PyObject) -> PyResult<i32> {
    def_action(sql, callback, bpe::def_mapper_ffi)
}

#[pyfunction]
#[allow(dead_code)]
fn def_mapper_bind_aggregate(sql: &str, aggregate_id: u16) -> PyResult<i32> {
    if let Some(id) = bpe::def_mapper_bind_aggregate(sql, aggregate_id) {
        Ok(id as i32)
    } else {
        Ok(-1)
    }
}

#[pyfunction]
#[allow(dead_code)]
fn def_aggregate(sql: &str, callback: PyObject) -> PyResult<i32> {
    def_action(sql, callback, bpe::def_aggregate_ffi)
}

/// Builds a Window from the int encoding: 0=None, 1=Tumbling{period_ms},
/// 2=Sliding{length_ms, slide_ms}.
fn conv_window(window_type: u16, period_ms: u64, length_ms: u64, slide_ms: u64) -> Option<Window> {
    match window_type {
        0 => Some(Window::None),
        1 => Some(Window::Tumbling { period_ms }),
        2 => Some(Window::Sliding { length_ms, slide_ms }),
        _ => {
            log::warn!("unknown window_type: {window_type}");
            None
        }
    }
}

#[pyfunction]
#[allow(dead_code)]
#[pyo3(signature = (sql, window_type, period_ms, length_ms, slide_ms, ts_field, lag_ms, callback))]
fn def_window_aggregate(
    sql: &str,
    window_type: u16,
    period_ms: u64,
    length_ms: u64,
    slide_ms: u64,
    ts_field: Option<&str>,
    lag_ms: u64,
    callback: PyObject,
) -> PyResult<i32> {
    let window = match conv_window(window_type, period_ms, length_ms, slide_ms) {
        Some(w) => w,
        None => return Ok(-1),
    };
    if let Some(id) = bpe::def_window_aggregate_ffi(
        sql,
        window,
        ts_field,
        lag_ms,
        Box::new(PythonWindowFfiFunc { callback }),
    ) {
        Ok(id as i32)
    } else {
        Ok(-1)
    }
}

#[pyfunction]
#[allow(dead_code)]
#[pyo3(signature = (sql, window_type, period_ms, length_ms, slide_ms, ts_field, key_field, lag_ms, callback))]
fn def_keyed_window_aggregate(
    sql: &str,
    window_type: u16,
    period_ms: u64,
    length_ms: u64,
    slide_ms: u64,
    ts_field: Option<&str>,
    key_field: &str,
    lag_ms: u64,
    callback: PyObject,
) -> PyResult<i32> {
    let window = match conv_window(window_type, period_ms, length_ms, slide_ms) {
        Some(w) => w,
        None => return Ok(-1),
    };
    if let Some(id) = bpe::def_keyed_window_aggregate_ffi(
        sql,
        window,
        ts_field,
        Some(key_field),
        lag_ms,
        Box::new(PythonWindowFfiFunc { callback }),
    ) {
        Ok(id as i32)
    } else {
        Ok(-1)
    }
}

#[pyfunction]
#[allow(dead_code)]
fn def_dimension() -> PyResult<i32> {
    Ok(bpe::def_dimension().map(|id| id as i32).unwrap_or(-1))
}

#[pyfunction]
#[allow(dead_code)]
fn update_dimension(id: u16, key: i64, value: i64) {
    bpe::update_dimension(id, key, value);
}

#[pyfunction]
#[allow(dead_code)]
fn remove_dimension(id: u16, key: i64) {
    bpe::remove_dimension(id, key);
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
fn bpe4py(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(start, m)?)?;
    m.add_function(wrap_pyfunction!(stop, m)?)?;
    m.add_function(wrap_pyfunction!(set_delivery_mode, m)?)?;
    m.add_function(wrap_pyfunction!(dropped_events, m)?)?;
    m.add_function(wrap_pyfunction!(def_incoming, m)?)?;
    m.add_function(wrap_pyfunction!(def_stream, m)?)?;
    m.add_function(wrap_pyfunction!(new_data, m)?)?;
    m.add_function(wrap_pyfunction!(def_mapper, m)?)?;
    m.add_function(wrap_pyfunction!(def_mapper_bind_aggregate, m)?)?;
    m.add_function(wrap_pyfunction!(def_aggregate, m)?)?;
    m.add_function(wrap_pyfunction!(def_window_aggregate, m)?)?;
    m.add_function(wrap_pyfunction!(def_keyed_window_aggregate, m)?)?;
    m.add_function(wrap_pyfunction!(def_dimension, m)?)?;
    m.add_function(wrap_pyfunction!(update_dimension, m)?)?;
    m.add_function(wrap_pyfunction!(remove_dimension, m)?)?;
    Ok(())
}

struct PythonFfiFunc {
    callback: PyObject,
}
impl FfiFunc for PythonFfiFunc {
    fn callback(&self, params: CallbackParams) {
        let data_ptr = params.u8_ptr();
        let size = params.size();
        Python::with_gil(|py| {
            let data = unsafe {
                let array_ptr = data_ptr as *const [u8; U8_DATA_MAX_SIZE];
                slice::from_raw_parts(array_ptr, size)
            };
            call_py_func(py, &self.callback, data);
        })
    }
}

fn call_py_func(py: Python, callback: &PyObject, data: &[[u8; U8_DATA_MAX_SIZE]]) {
    if let Ok(func) = callback.getattr(py, "callback") {
        let array = conv_array(data);
        let args = (array,);
        let rs = func.call1(py, args);
        if rs.is_err() {
            log::warn!("call python method failed: {:?}", rs.err().unwrap());
        }
    }
}

/// Window callbacks receive `(data, window_start_ms, window_end_ms)` so the
/// Python side knows which window produced the result.
struct PythonWindowFfiFunc {
    callback: PyObject,
}
impl FfiFunc for PythonWindowFfiFunc {
    fn callback(&self, params: CallbackParams) {
        let data_ptr = params.u8_ptr();
        let size = params.size();
        let start = params.window_start_ms();
        let end = params.window_end_ms();
        Python::with_gil(|py| {
            let data = unsafe {
                let array_ptr = data_ptr as *const [u8; U8_DATA_MAX_SIZE];
                slice::from_raw_parts(array_ptr, size)
            };
            if let Ok(func) = self.callback.getattr(py, "callback") {
                let array = conv_array(data);
                let args = (array, start, end);
                let rs = func.call1(py, args);
                if rs.is_err() {
                    log::warn!("call python window callback failed: {:?}", rs.err().unwrap());
                }
            }
        })
    }
}

fn conv_array(data: &[[u8; U8_DATA_MAX_SIZE]]) -> Vec<u8> {
    let len = data.len();
    let mut vec = vec![0_u8; len * U8_DATA_MAX_SIZE];
    for i in 0..len {
        vec[i * U8_DATA_MAX_SIZE..(i + 1) * U8_DATA_MAX_SIZE].copy_from_slice(&data[i]);
    }
    vec
}
