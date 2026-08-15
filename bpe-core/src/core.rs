use crate::{
    aggregate::{define_aggregate, init_aggregate, search_aggregate},
    callback::FnHolder,
    cfg::load_config,
    data::{check_id_in_store, init_data, Column, Record, RecordType, U8Bytes},
    ffi::FfiFunc,
    jit::base::init_func_generator,
    logger::init_logger,
    mapper::{call_mapper, define_mapper, define_mapper_bind_aggregate, init_mapper},
    param::CallbackParams,
    store::{find_or_insert_array, get_record_size, init_store, insert, WrappedArray},
    window::{define_keyed_window_aggregate, define_window_aggregate, Window},
};
use std::sync::Once;

/// Initializes the BPE system by setting up configuration, logger, function generator,
/// data structures, store, mapper, and aggregate components.
/// This function ensures initialization happens only once using std::sync::Once.
pub fn start() {
    static START: Once = Once::new();
    START.call_once(actual_start);
}

/// Internal function that performs the actual initialization of all BPE components
/// in the correct order: config, logger, function generator, data, store, mapper, and aggregate.
fn actual_start() {
    load_config(); // Load configuration settings
    init_logger(); // Initialize logging system
    init_func_generator(); // Initialize JIT function generator
    init_data(); // Initialize data structures
    init_store(); // Initialize data store
    init_mapper(); // Initialize mapper components
    init_aggregate(); // Initialize aggregate components
}

/// Stops the BPE system by marking it as deactivated.
/// This function ensures deactivation happens only once using std::sync::Once.
pub fn stop() {
    static STOP: Once = Once::new();
    STOP.call_once(actual_stop);
}

/// Internal function that logs the deactivation of the BPE system.
fn actual_stop() {
    crate::window::stop_timer(); // stop the window timer thread
    log::info!("mark as deactived");
}

/// Defines an incoming record with the specified name and column structure.
/// Returns the assigned record ID if successful, or None if the record could not be created.
pub fn def_incoming(name: &str, columns: Vec<Column>) -> Option<u16> {
    Record::insert_record(name, RecordType::Incoming, columns)
}

/// Defines a stream record with the specified name and column structure.
/// Returns the assigned record ID if successful, or None if the record could not be created.
pub fn def_stream(name: &str, columns: Vec<Column>) -> Option<u16> {
    Record::insert_record(name, RecordType::Stream, columns)
}

/// Processes new incoming data by checking if the record ID is defined and the payload
/// fits into one record slot, inserting it into the appropriate array, and triggering
/// mapper processing. Returns true if the data was successfully processed, false otherwise.
pub fn new_data(data: &U8Bytes) -> bool {
    let id = data.id(); // Extract the record ID from the data
    if check_id_in_store(id) {
        log::warn!("id [{id}] is not defined");
        false
    } else if data.data_len() > get_record_size() {
        // guard against writing beyond the record slot (cross-record corruption)
        log::warn!(
            "data_len [{}] is larger than record_size [{}], data is rejected",
            data.data_len(),
            get_record_size()
        );
        false
    } else {
        let array = find_or_insert_array(id); // Find or create storage array for this record ID
        process_data(array, data); // Insert data and trigger mapper processing
        crate::window::on_new_data(id, data); // place into time-window buckets (cheap fast path)
        true
    }
}

/// Processes data by inserting it into the array and triggering the mapper.
/// This function is marked as inline for performance optimization.
#[inline]
fn process_data(array: &mut WrappedArray, data: &U8Bytes) {
    insert(array, data); // Insert the data into the storage array
    call_mapper(array, data.id()); // Trigger mapper processing for this record ID
}

/// Defines a mapper function that will be executed when data matching the SQL query is available.
/// Takes a SQL query string and a callback function that processes the data.
/// Returns the mapper ID if successful, or None if the mapper could not be created.
pub fn def_mapper<F>(sql: &str, func: F) -> Option<u16>
where
    F: Fn(CallbackParams) + Send + 'static,
{
    define_mapper(sql, FnHolder::Func(Box::new(func))) // Wrap the function and register the mapper
}

/// Defines a mapper that is bound to an aggregate function.
/// When the mapper is triggered, it will call the specified aggregate function with the provided parameters.
/// Returns the mapper ID if successful, or None if the aggregate could not be found or mapper could not be created.
pub fn def_mapper_bind_aggregate(sql: &str, aggregate_id: u16) -> Option<u16> {
    let opt_aggregate = search_aggregate(aggregate_id); // Look up the aggregate by ID
    if let Some(wrapped) = opt_aggregate {
        define_mapper_bind_aggregate(sql, wrapped)
    } else {
        None // Return None if the aggregate ID was not found
    }
}

/// Defines a mapper that uses a foreign function interface (FFI) function.
/// This allows external functions (potentially from other languages) to be used as mapper callbacks.
/// Returns the mapper ID if successful, or None if the mapper could not be created.
pub fn def_mapper_ffi(sql: &str, ffi: Box<dyn FfiFunc>) -> Option<u16> {
    define_mapper(sql, FnHolder::FfiFunc(ffi)) // Register the mapper with the FFI function
}

/// Defines an aggregate function that will be executed when data matching the SQL query is available.
/// Aggregates are used to combine multiple data points into a single result.
/// Returns the aggregate ID if successful, or None if the aggregate could not be created.
pub fn def_aggregate<F>(sql: &str, func: F) -> Option<u16>
where
    F: Fn(CallbackParams) + Send + 'static,
{
    define_aggregate(sql, FnHolder::Func(Box::new(func))) // Wrap the function and register the aggregate
}

/// Defines a time-window aggregate: records passing the WHERE clause are placed
/// into time buckets (event time from `ts_field` if given, else processing time);
/// when a window's end time passes (plus `lag_ms` tolerance), the aggregate over
/// the window's records is computed and delivered to the callback.
///
/// The callback receives `CallbackParams` whose `u8_ptr` points to the aggregate
/// result (same layout as `def_aggregate`), `window_start_ms()/window_end_ms()`
/// carry the window time range, and `size()` is 1 when the window had records
/// (0 for an empty window, which still fires with initial values).
///
/// Note: the callback runs on the engine's window-timer thread, so it must be
/// `Send` and should not block for long.
pub fn def_window_aggregate<F>(
    sql: &str,
    window: Window,
    ts_field: Option<&str>,
    lag_ms: u64,
    func: F,
) -> Option<u16>
where
    F: Fn(CallbackParams) + Send + 'static,
{
    define_window_aggregate(sql, window, ts_field, lag_ms, FnHolder::Func(Box::new(func)))
}

/// FFI variant of `def_window_aggregate` for language bindings.
pub fn def_window_aggregate_ffi(
    sql: &str,
    window: Window,
    ts_field: Option<&str>,
    lag_ms: u64,
    ffi: Box<dyn FfiFunc>,
) -> Option<u16> {
    define_window_aggregate(sql, window, ts_field, lag_ms, FnHolder::FfiFunc(ffi))
}

/// FFI variant of `def_keyed_window_aggregate` for language bindings.
pub fn def_keyed_window_aggregate_ffi(
    sql: &str,
    window: Window,
    ts_field: Option<&str>,
    key_field: Option<&str>,
    lag_ms: u64,
    ffi: Box<dyn FfiFunc>,
) -> Option<u16> {
    match key_field {
        Some(k) => define_keyed_window_aggregate(sql, window, ts_field, k, lag_ms, FnHolder::FfiFunc(ffi)),
        None => {
            log::warn!("def_keyed_window_aggregate_ffi requires a key_field");
            None
        }
    }
}

/// Defines a per-key time-window aggregate: like `def_window_aggregate`, but the
/// window result is delivered once per distinct value of the `key_field` column.
///
/// The callback receives rows laid out as `[key i64][field0]...[fieldN]` per key;
/// `size()` is the number of keys in the window and `step()` is `(1+N)*8`.
/// An empty window (no keys) still fires with `size()==0`.
pub fn def_keyed_window_aggregate<F>(
    sql: &str,
    window: Window,
    ts_field: Option<&str>,
    key_field: &str,
    lag_ms: u64,
    func: F,
) -> Option<u16>
where
    F: Fn(CallbackParams) + Send + 'static,
{
    define_keyed_window_aggregate(
        sql,
        window,
        ts_field,
        key_field,
        lag_ms,
        FnHolder::Func(Box::new(func)),
    )
}

/// Defines an aggregate function that uses a foreign function interface (FFI) function.
/// This allows external functions (potentially from other languages) to be used as aggregate callbacks.
/// Returns the aggregate ID if successful, or None if the aggregate could not be created.
pub fn def_aggregate_ffi(sql: &str, ffi: Box<dyn FfiFunc>) -> Option<u16> {
    define_aggregate(sql, FnHolder::FfiFunc(ffi)) // Register the aggregate with the FFI function
}
