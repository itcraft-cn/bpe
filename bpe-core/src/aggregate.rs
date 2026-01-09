use crate::{
    agg_func,
    aux::{fetch_ptr, fill_ptr, SimpleU16Map},
    callback::{callback, FnHolder},
    consts::U8_DATA_MAX_SIZE,
    data::{ColumnType, Record},
    element::Element,
    error::ParseSqlError,
    exec::{create_executor, Executor},
    func_enum::SupportFunc,
    id::next_aggregate_id,
    param::CallbackParams,
    sql::{
        base::{parse_options, ParsedSql},
        select::parse_select,
    },
};
use globalvar::{def_global_ptr, get_global, get_global_mut};
use std::alloc::{self, Layout};

static mut PTR_AGGREGATE_MAP: u64 = 0;
static mut PTR_PARSE_OPTIONS: u64 = 0;
static mut PTR_VAL_DATA_REF: u64 = 0;

/// Initializes the aggregate system by setting up global pointers for:
/// - Aggregate map to store aggregate definitions
/// - Parse options for SQL parsing
/// - Value data reference for aggregate computations
pub(crate) fn init_aggregate() {
    unsafe {
        PTR_AGGREGATE_MAP = def_global_ptr(SimpleU16Map::new()); // Initialize map to store aggregates
        PTR_PARSE_OPTIONS = def_global_ptr(parse_options()); // Initialize SQL parsing options
                                                             // Allocate memory for aggregate data reference
        PTR_VAL_DATA_REF =
            alloc::alloc(Layout::from_size_align(U8_DATA_MAX_SIZE, 1).unwrap()) as u64;
    }
}

/// Defines an aggregate function by parsing the SQL query, generating the aggregate structure,
/// and storing it in the aggregate map for later use.
/// Returns Some(1) if successful, or None if the aggregate could not be created.
pub(crate) fn define_aggregate(sql: &str, func_holder: FnHolder) -> Option<u16> {
    let options = get_global(unsafe { PTR_PARSE_OPTIONS }); // Get global parse options
    if let Some(parsed_sql) = parse_select(sql, options) {
        // Parse the SQL query
        let rs = gen_aggregate(&parsed_sql); // Generate aggregate structure
        if let Ok(aggregate) = rs {
            let map = get_global_mut::<SimpleU16Map>(unsafe { PTR_AGGREGATE_MAP }); // Get aggregate map
                                                                                    // Store the aggregate with its function holder in the map
            map.insert(
                aggregate.id(),
                WrappedAggregate::new(aggregate, func_holder),
            );
            Some(1) // Return success indicator
        } else {
            log::warn!("{:?}", rs.err()); // Log error if aggregate generation failed
            None
        }
    } else {
        None // Return None if SQL parsing failed
    }
}

/// Calls an aggregate function with the provided parameters.
/// This function retrieves the stream associated with the aggregate and prepares the data pointer
/// for aggregate computation.
#[inline]
pub(crate) fn call_aggregate(wrapped: &WrappedAggregate, param: CallbackParams) {
    let id = wrapped.aggregate().stream_id(); // Get the stream ID from the aggregate
    if let Some(stream) = Record::get_record(id) {
        // Look up the stream by ID
        // Get pointer to aggregate data reference
        let aggregate_data_ptr = unsafe { PTR_VAL_DATA_REF } as *mut u8;
        call_with_aggregate_data(aggregate_data_ptr, wrapped, stream, param); // Execute aggregate
    } else {
        log::warn!("failed to find stream by id[{id}]"); // Log warning if stream not found
    }
}

/// Executes aggregate computation by first initializing the data, then computing the result,
/// and finally calling the callback function with the computed data.
#[inline]
fn call_with_aggregate_data(
    aggregate_data_ptr: *mut u8, // Pointer to memory where aggregate data will be stored
    wrapped: &WrappedAggregate,  // The wrapped aggregate structure
    stream: &Record,             // The stream record containing column information
    param: CallbackParams,       // Parameters for the callback
) {
    if init_data(aggregate_data_ptr, wrapped, stream) {
        // Initialize aggregate data
        // Compute the aggregate result using the input parameters
        compute_data(aggregate_data_ptr, wrapped, stream, param);
        // Call the callback function with the computed aggregate data
        callback(
            &wrapped.fn_holder,
            CallbackParams::new(aggregate_data_ptr, 1, 0, 1, 1),
        );
    }
}

/// Initializes aggregate data by setting up initial values for each executor in the aggregate.
/// Returns true if all initializations succeed, false if any fail.
#[inline]
fn init_data(aggregate_data_ptr: *mut u8, wrapped: &WrappedAggregate, stream: &Record) -> bool {
    // Iterate through each executor in the aggregate with its column index
    for (col_idx, executor) in wrapped.aggregate().executors().iter().enumerate() {
        // Set up the initial value for this executor
        let rs = setup_init_val(aggregate_data_ptr, col_idx, executor, stream);
        if rs.is_err() {
            log::warn!("hit error: {:?}", rs.err()); // Log error if initialization failed
            return false; // Return false to indicate failure
        }
    }
    true // Return true to indicate all initializations succeeded
}

#[inline]
fn setup_init_val(
    aggregate_data_ptr: *mut u8,
    col_idx: usize,
    executor: &Executor,
    stream: &Record,
) -> Result<(), String> {
    let offset = stream.column((col_idx + 1) as u16).offset();
    match executor {
        Executor::ConstLong(_) => Ok(()),
        Executor::ConstDouble(_) => Ok(()),
        Executor::Compute(func, _) => {
            init_for_some_func(func, aggregate_data_ptr, offset);
            Ok(())
        }
        _ => {
            log::warn!(
                "expr in top level just support aggregate func, this is not aggregate func:{executor:?}"
            );
            Err(String::from("not aggregate func"))
        }
    }
}

#[inline]
fn init_for_some_func(func: &SupportFunc, aggregate_data_ptr: *mut u8, offset: usize) {
    match func {
        SupportFunc::MaxL => {
            fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, i64::MIN);
        }
        SupportFunc::MinL => {
            fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, i64::MAX);
        }
        SupportFunc::MaxD => {
            fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, f64::MIN);
        }
        SupportFunc::MinD => {
            fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, f64::MAX);
        }
        _ => {}
    }
}

/// Computes aggregate data by processing each executor in the aggregate.
/// Handles constant values, single-argument functions (First/Last), and multi-argument functions.
#[inline]
fn compute_data(
    aggregate_data_ptr: *mut u8, // Pointer to memory where aggregate data is stored
    wrapped: &WrappedAggregate,  // The wrapped aggregate structure
    stream: &Record,             // The stream record containing column information
    param: CallbackParams,       // Parameters for computation
) {
    // Create a wrapped parameter structure for aggregate computation
    let wrapped_agg_param = WrappedAggParam::new(aggregate_data_ptr, stream);
    // Process each executor in the aggregate with its column index
    for (col_idx, executor) in wrapped.aggregate().executors().iter().enumerate() {
        // Calculate the memory offset for this column
        let offset = stream.column((col_idx + 1) as u16).offset();
        match executor {
            Executor::ConstLong(v) => {
                // Handle constant long value by filling the memory at offset
                fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, *v);
            }
            Executor::ConstDouble(v) => {
                // Handle constant double value by filling the memory at offset
                fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, *v);
            }
            Executor::Compute(func, executors) => {
                // Get the number of executors for this function
                let executor_size = executors.executor_size();
                if executor_size != 1 {
                    // Log warning if function doesn't support the number of arguments
                    log::warn!("aggregate func[{func:?}] just support one argument, here is {executor_size:?} executors");
                    return; // Return early if argument count is incorrect
                }
                match func {
                    SupportFunc::FirstL | SupportFunc::FirstD => {
                        // Handle First functions: get the first value
                        let sub_executor = executors.index_of(0); // Get the first sub-executor
                        let element = fetch_arg_val(param.u8_ptr(), sub_executor, stream); // Get value
                        call_once_compute(&wrapped_agg_param, func, element, offset);
                        // Compute
                    }
                    SupportFunc::LastL | SupportFunc::LastD => {
                        // Handle Last functions: get the last value
                        let sub_executor = executors.index_of(0); // Get the first sub-executor
                        let last = param.size() - 1; // Calculate index of last element
                                                     // Get pointer to the last data element
                        let sub_data = unsafe { param.u8_ptr().add(last * param.step()) };
                        let element = fetch_arg_val(sub_data, sub_executor, stream); // Get value
                        call_once_compute(&wrapped_agg_param, func, element, offset);
                        // Compute
                    }
                    _ => loop_compute(aggregate_data_ptr, stream, col_idx, executor, &param), // Handle other functions
                }
            }
            _ => {}
        }
    }
}

fn call_once_compute(
    wrapped_agg_param: &WrappedAggParam,
    func: &SupportFunc,
    element: Element,
    offset: usize,
) {
    choose_func(func, element, wrapped_agg_param, offset, 0);
}

fn loop_compute(
    aggregate_data_ptr: *mut u8,
    stream: &Record,
    col_idx: usize,
    executor: &Executor,
    param: &CallbackParams,
) {
    let u8_ptr = param.u8_ptr();
    let size = param.size();
    let wrapped_agg_param = WrappedAggParam::new(aggregate_data_ptr, stream);
    for data_idx in 0..size {
        compute(
            col_idx,
            &wrapped_agg_param,
            executor,
            unsafe { u8_ptr.add(data_idx * param.step()) },
            data_idx,
        );
    }
}

#[inline]
fn compute(
    col_idx: usize,
    wrapped_agg_param: &WrappedAggParam,
    executor: &Executor,
    sub_data: *const u8,
    data_idx: usize,
) {
    let stream = wrapped_agg_param.stream();
    let offset = stream.column((col_idx + 1) as u16).offset();
    match executor {
        Executor::Compute(func, executors) => {
            let sub_executor = executors.index_of(0);
            let element = fetch_arg_val(sub_data, sub_executor, stream);
            choose_func(func, element, wrapped_agg_param, offset, data_idx);
        }
        _ => {
            log::warn!(
                "expr in top level just support aggregate func, this is not aggregate func:{executor:?}"
            );
        }
    }
}

#[inline]
fn choose_func(
    func: &SupportFunc,
    element: Element,
    wrapped_agg_param: &WrappedAggParam,
    offset: usize,
    data_idx: usize,
) {
    let aggregate_data_ptr = wrapped_agg_param.aggregate_data_ptr();
    match func {
        SupportFunc::MaxL => match &element {
            Element::Long(v) => {
                agg_func::func_max_long(aggregate_data_ptr, offset, v);
            }
            _ => {
                log::warn!("unsupported function: {:?}-{:?}", func, &element);
            }
        },
        SupportFunc::MinL => match &element {
            Element::Long(v) => {
                agg_func::func_min_long(aggregate_data_ptr, offset, v);
            }
            _ => {
                log::warn!("unsupported function: {:?}-{:?}", func, &element);
            }
        },
        SupportFunc::SumL => match &element {
            Element::Long(v) => {
                agg_func::func_sum_long(aggregate_data_ptr, offset, v);
            }
            _ => {
                log::warn!("unsupported function: {:?}-{:?}", func, &element);
            }
        },
        SupportFunc::Count => match &element {
            Element::Long(_) => {
                agg_func::func_count_long(aggregate_data_ptr, offset);
            }
            _ => {
                log::warn!("unsupported function: {:?}-{:?}", func, &element);
            }
        },
        SupportFunc::MaxD => match &element {
            Element::Long(v) => {
                agg_func::func_maxd_long(aggregate_data_ptr, offset, v);
            }
            Element::Double(v) => {
                agg_func::func_maxd_double(aggregate_data_ptr, offset, v);
            }
        },
        SupportFunc::MinD => match &element {
            Element::Long(v) => {
                agg_func::func_mind_long(aggregate_data_ptr, offset, v);
            }
            Element::Double(v) => {
                agg_func::func_mind_double(aggregate_data_ptr, offset, v);
            }
        },
        SupportFunc::SumD => match &element {
            Element::Long(v) => {
                agg_func::func_sumd_long(aggregate_data_ptr, offset, v);
            }
            Element::Double(v) => {
                agg_func::func_sumd_double(aggregate_data_ptr, offset, v);
            }
        },
        SupportFunc::Avg => match &element {
            Element::Long(v) => {
                agg_func::func_avg_long(aggregate_data_ptr, offset, v, data_idx);
            }
            Element::Double(v) => {
                agg_func::func_avg_double(aggregate_data_ptr, offset, v, data_idx);
            }
        },
        SupportFunc::FirstL => match &element {
            Element::Long(v) => {
                agg_func::func_first_long(aggregate_data_ptr, offset, v);
            }
            _ => {
                log::warn!("unsupported function: {:?}-{:?}", func, &element);
            }
        },
        SupportFunc::FirstD => {
            let v = match &element {
                Element::Long(v) => *v as f64,
                Element::Double(v) => *v,
            };
            agg_func::func_first_double(aggregate_data_ptr, offset, &v);
        }
        SupportFunc::LastL => match &element {
            Element::Long(v) => {
                agg_func::func_last_long(aggregate_data_ptr, offset, v);
            }
            _ => {
                log::warn!("unsupported function: {:?}-{:?}", func, &element);
            }
        },
        SupportFunc::LastD => {
            let v = match &element {
                Element::Long(v) => *v as f64,
                Element::Double(v) => *v,
            };
            agg_func::func_last_double(aggregate_data_ptr, offset, &v);
        }
        _ => {
            log::warn!("unsupported function: {:?}-{:?}", func, &element);
        }
    };
}

fn fetch_arg_val(sub_data: *const u8, executor: &Executor, stream: &Record) -> Element {
    match executor {
        Executor::Fetch(_record_id, field_id) => {
            let column = stream.column(*field_id);
            let column_type = column.data_type();
            match column_type {
                ColumnType::Long => {
                    Element::Long(unsafe { fetch_ptr(sub_data.add(column.offset())) })
                }
                ColumnType::Double => {
                    Element::Double(unsafe { fetch_ptr(sub_data.add(column.offset())) })
                }
            }
        }
        _ => {
            log::warn!("just support fetch, here is {executor:?} executor");
            Element::Long(0)
        }
    }
}

pub(crate) fn search_aggregate(id: u16) -> Option<&'static WrappedAggregate> {
    let map = get_global::<SimpleU16Map>(unsafe { PTR_AGGREGATE_MAP });
    map.get(id)
}

fn gen_aggregate(parsed_sql: &ParsedSql) -> Result<Aggregate, ParseSqlError> {
    let fields = parsed_sql.fields();
    if fields.is_empty() {
        return Err(ParseSqlError::new(String::from("no field in aggregate")));
    }
    if parsed_sql.records().len() != 1 {
        return Err(ParseSqlError::new(String::from(
            "only support one record in aggregate",
        )));
    }
    let opt_stream = Record::get_record(parsed_sql.records()[0]);
    if opt_stream.is_none() {
        return Err(ParseSqlError::new(format!(
            "stream {} not found",
            parsed_sql.records()[0]
        )));
    }
    let rs = create_executor(parsed_sql.records(), fields);
    if let Ok(executors) = rs {
        let id = next_aggregate_id();
        Ok(Aggregate {
            id,
            stream_id: opt_stream.unwrap().id(),
            executors,
        })
    } else {
        Err(ParseSqlError::new(String::from(
            "fail to create executors for aggregate",
        )))
    }
}

pub(crate) struct Aggregate {
    id: u16,
    stream_id: u16,
    executors: Vec<Executor>,
}
impl Aggregate {
    fn id(&self) -> u16 {
        self.id
    }
    fn stream_id(&self) -> u16 {
        self.stream_id
    }
    fn executors(&self) -> &Vec<Executor> {
        &self.executors
    }
}

pub(crate) struct WrappedAggregate {
    aggregate: Aggregate,
    fn_holder: FnHolder,
}
impl WrappedAggregate {
    fn new(aggregate: Aggregate, fn_holder: FnHolder) -> Self {
        WrappedAggregate {
            aggregate,
            fn_holder,
        }
    }
    fn aggregate(&self) -> &Aggregate {
        &self.aggregate
    }
}

struct WrappedAggParam<'a> {
    aggregate_data_ptr: *mut u8,
    stream: &'a Record,
}
impl<'a> WrappedAggParam<'a> {
    fn new(aggregate_data_ptr: *mut u8, stream: &'a Record) -> Self {
        WrappedAggParam {
            aggregate_data_ptr,
            stream,
        }
    }

    fn aggregate_data_ptr(&self) -> *mut u8 {
        self.aggregate_data_ptr
    }

    fn stream(&self) -> &'a Record {
        self.stream
    }
}
