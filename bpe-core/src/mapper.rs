use crate::{
    aux::{SimpleU16Entry, SimpleU16Map},
    callback::{callback, FnHolder},
    data::Record,
    error::ParseSqlError,
    exec::{create_executor, Executors},
    id::next_mapper_id,
    param::CallbackParams,
    sql::{
        base::{parse_options, FilterFunc, ParsedSql},
        select::parse_select,
    },
    store::{get_vec_size, WrappedArray},
};
use globalvar::{def_global_ptr, get_global, get_global_mut};
use inkwell::execution_engine::JitFunction;
use std::alloc::{self, Layout};

const OPERATOR_INC: fn(usize) -> usize = |v| v + 1;
const OPERATOR_DEC: fn(usize) -> usize = |v| v - 1;

static mut PTR_MAPPER_MAP: u64 = 0;
static mut PTR_PARSE_OPTIONS: u64 = 0;
static mut PTR_VAL_DATA_REF: u64 = 0;

/// Initializes the mapper system by setting up global pointers for:
/// - Mapper map to store mapper definitions
/// - Parse options for SQL parsing
/// - Value data reference for mapper computations
pub(crate) fn init_mapper() {
    unsafe {
        PTR_MAPPER_MAP = def_global_ptr(SimpleU16Map::new()); // Initialize map to store mappers
        PTR_PARSE_OPTIONS = def_global_ptr(parse_options()); // Initialize SQL parsing options
                                                             // Allocate memory for mapper data reference
        PTR_VAL_DATA_REF = alloc::alloc(Layout::from_size_align(get_vec_size(), 1).unwrap()) as u64;
    }
}

/// Defines a mapper function by parsing the SQL query, generating the mapper structure,
/// and storing it in the mapper map for later use.
/// Returns the mapper ID if successful, or None if the mapper could not be created.
pub(crate) fn define_mapper(sql: &str, func_holder: FnHolder) -> Option<u16> {
    let options = get_global(unsafe { PTR_PARSE_OPTIONS }); // Get global parse options
    if let Some(parsed_sql) = parse_select(sql, options) {
        // Parse the SQL query
        let rs = gen_mapper(parsed_sql); // Generate mapper structure
        if let Ok(mapper) = rs {
            let id = mapper.id(); // Get the mapper ID
            let mapper_map = get_global_mut::<SimpleU16Map>(unsafe { PTR_MAPPER_MAP }); // Get mapper map
            let entry = mapper_map.entry(id); // Check if ID already exists
            match entry {
                SimpleU16Entry::Exist(_) => {
                    log::warn!("id {id} already exists, sql[{sql}] is skipped"); // Log warning if duplicate
                    None
                }
                SimpleU16Entry::NotExist(_) => {
                    // Store the mapper with its function holder in the map
                    mapper_map.insert(id, WrappedMapper::new(mapper, func_holder));
                    Some(id) // Return the mapper ID
                }
            }
        } else {
            log::warn!(
                "fail to create mapper from sql[{}], hit unexpected error: {:?}",
                sql,
                rs.err().unwrap() // Log error if mapper generation failed
            );
            None
        }
    } else {
        log::warn!("not supported sql statement: [{sql}]"); // Log warning if SQL parsing failed
        None
    }
}

/// Calls a mapper function for the specified record ID by looking up the mapper and record,
/// then invoking the mapper with the provided array and parameters.
#[inline]
pub(crate) fn call_mapper(array: &WrappedArray, id: u16) {
    let opt_mappers = search_mapper(id); // Look up the mapper by ID
    let opt_record = Record::get_record(id); // Look up the record by ID
    if opt_mappers.is_none() || opt_record.is_none() {
        return; // Return early if either mapper or record is not found
    }
    let wrapped_mapper = opt_mappers.unwrap(); // Get the wrapped mapper
    let record = opt_record.unwrap(); // Get the record
                                      // Invoke the mapper with the array, mapper structure, record, and function holder
    invoke(
        id,
        array,
        &wrapped_mapper.mapper,
        record,
        &wrapped_mapper.fn_holder,
    );
}

fn search_mapper(id: u16) -> Option<&'static WrappedMapper> {
    let mapper_map = get_global::<SimpleU16Map>(unsafe { PTR_MAPPER_MAP });
    mapper_map.get(id)
}

fn gen_mapper(parsed_sql: ParsedSql) -> Result<Mapper, ParseSqlError> {
    let record_id_array = &parsed_sql.records();
    if record_id_array.len() != 1 {
        return Err(ParseSqlError::new(format!(
            "only support one record, but {} records",
            record_id_array.len()
        )));
    }
    let id = next_mapper_id();
    let rs_executors = create_executor(record_id_array, parsed_sql.fields());
    if rs_executors.is_err() {
        return Err(ParseSqlError::new(format!(
            "failed to parse executors: {}",
            rs_executors.err().unwrap()
        )));
    }
    let vec_executors = rs_executors.unwrap();
    Ok(Mapper::new(
        id,
        parsed_sql,
        Executors::new(vec_executors.as_slice()),
    ))
}

/// Invokes a mapper by filtering data in the array, then calling the callback function
/// with the filtered results and appropriate parameters.
#[inline]
fn invoke(
    id: u16,                 // Record ID
    array: &WrappedArray,    // Array containing the data
    mapper: &'static Mapper, // The mapper to invoke
    record: &Record,         // The record definition
    fn_holder: &FnHolder,    // The function to call with results
) {
    // Get pointer to value data reference for processing
    let u8_ptr = unsafe { PTR_VAL_DATA_REF } as *mut u8;
    // Filter data in the array based on the mapper's criteria
    let size = loop_filter(array, mapper, id, record, u8_ptr);
    let mask = array.mask(); // Get the array mask
    let offset = (array.walker() - 1) & mask; // Calculate the offset
    let step = array.step(); // Get the step size
                             // Call the callback function with the processed data and parameters
    callback(
        fn_holder,
        CallbackParams::new(u8_ptr, mask, offset, size, step),
    );
}

/// Filters data in the array based on the mapper's criteria, processing elements that match
/// and returning the count of processed elements.
fn loop_filter(
    array: &WrappedArray, // Array containing the data to filter
    mapper: &Mapper,      // The mapper with filter criteria
    id: u16,              // Record ID
    record: &Record,      // The record definition
    u8_ptr: *mut u8,      // Pointer to output buffer
) -> usize {
    let walker = array.walker(); // Get the array walker position
    let mask = array.mask(); // Get the array mask
    let first_idx = array.first_idx(); // Get first index in array
    let last_idx = array.last_idx(); // Get last index in array
    let step = array.step(); // Get the step size between elements
    let v_ptr = array.u64ptr(); // Get the array's u64 pointer
    let mut n = 0; // Counter for processed elements
    let mut offset = 0; // Offset in output buffer
    let limit = mapper.limit(); // Get the limit on number of results
    let asc = mapper.fetch_asc(); // Check if fetching in ascending order
                                  // Create an index wrapper that iterates in the appropriate direction
    let mut idx_wrapper = if asc {
        Idx::new(last_idx, first_idx, OPERATOR_DEC) // Descending order
    } else {
        Idx::new(first_idx, last_idx, OPERATOR_INC) // Ascending order
    };
    let filter = mapper.filter(); // Get the filter function
    loop {
        // Calculate the position in the array
        let position = ((walker - 1 - idx_wrapper.idx()) * step) & mask;
        let sub_data_ptr = array.sub_data(position); // Get pointer to sub-data
        let v_sub_ptr = sub_data_ptr as u64; // Convert to u64 pointer
        if unsafe { filter.call(v_sub_ptr) } {
            // Apply the filter
            let adjusted = unsafe { u8_ptr.add(offset) }; // Adjust output pointer
                                                          // Fetch and copy the data that passed the filter
            mapper.fetch(id, v_ptr, record, position, sub_data_ptr, adjusted);
            n += 1; // Increment processed element counter
            offset += step; // Update output buffer offset
            if n == limit {
                // Check if we've reached the limit
                break;
            }
        }
        if idx_wrapper.judge_or_step() {
            // Check if we should continue iterating
            break;
        }
    }
    n // Return the count of processed elements
}

#[derive(Debug)]
pub(crate) struct Mapper {
    id: u16,
    parsed_sql: ParsedSql,
    executors: Executors,
}
impl Mapper {
    fn new(id: u16, parsed_sql: ParsedSql, executors: Executors) -> Mapper {
        Self {
            id,
            parsed_sql,
            executors,
        }
    }

    pub(crate) fn id(&self) -> u16 {
        self.id
    }

    fn fetch(
        &self,
        id: u16,
        v_ptr: u64,
        record: &Record,
        position: usize,
        sub_data_ptr: *const u8,
        target: *const u8,
    ) {
        let mut val;
        let mut offset = 0_usize;
        let executors = &self.executors;
        let len = executors.executor_size();
        let mut i = 0;
        loop {
            let executor = executors.index_of(i);
            val = executor.fetch(id, v_ptr, record, position, sub_data_ptr);
            let val_len = val.len();
            val.copy_to_target(target as *mut u8, offset);
            offset += val_len;
            i += 1;
            if i >= len {
                break;
            }
        }
    }

    fn filter(&self) -> &JitFunction<'_, FilterFunc> {
        self.parsed_sql.filter()
    }

    fn limit(&self) -> usize {
        self.parsed_sql.limit()
    }

    fn fetch_asc(&self) -> bool {
        self.parsed_sql.fetch_asc()
    }
}

pub(crate) struct WrappedMapper {
    mapper: Mapper,
    fn_holder: FnHolder,
}
impl WrappedMapper {
    fn new(mapper: Mapper, fn_holder: FnHolder) -> Self {
        WrappedMapper { mapper, fn_holder }
    }
}

#[derive(Debug)]
struct Idx {
    idx: usize,
    stop_val: usize,
    operator: fn(usize) -> usize,
}
impl Idx {
    fn new(init_val: usize, stop_val: usize, operator: fn(usize) -> usize) -> Self {
        Self {
            idx: init_val,
            stop_val,
            operator,
        }
    }

    fn idx(&self) -> usize {
        self.idx
    }

    fn judge_or_step(&mut self) -> bool {
        if self.idx == self.stop_val {
            true
        } else {
            self.idx = (self.operator)(self.idx);
            false
        }
    }
}
