use crate::{
    aggregate::{call_aggregate, WrappedAggregate},
    aux::{SimpleU16Entry, SimpleU16Map},
    callback::{callback, FnHolder},
    consts::FIELD_SIZE,
    data::Record,
    error::ParseSqlError,
    exec::{create_executor, Executors},
    id::next_mapper_id,
    param::CallbackParams,
    sql::{
        base::{parse_options, ExprEntity, FilterFunc, ParsedSql},
        select::parse_select,
    },
    store::{get_vec_size, WrappedArray},
};
use globalvar::{def_global_ptr, get_global, get_global_mut};
use inkwell::execution_engine::JitFunction;
use std::{alloc::{self, Layout}, sync::Arc};

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
/// The mapper map is keyed by the target RECORD id (not the mapper id), so multiple
/// mappers can be registered for the same record and all of them are invoked.
/// Returns the mapper ID if successful, or None if the mapper could not be created.
pub(crate) fn define_mapper(sql: &str, func_holder: FnHolder) -> Option<u16> {
    let options = get_global(unsafe { PTR_PARSE_OPTIONS }); // Get global parse options
    if let Some(parsed_sql) = parse_select(sql, options) {
        // Parse the SQL query
        let rs = gen_mapper(parsed_sql); // Generate mapper structure
        if let Ok(mapper) = rs {
            register_mapper(mapper, func_holder)
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

/// Defines a mapper that is bound to an aggregate function.
/// The aggregate field references are resolved against the mapper's SELECT output
/// by field NAME, so the aggregate reads the correct mapper output columns.
pub(crate) fn define_mapper_bind_aggregate(
    sql: &str,
    wrapped: &'static WrappedAggregate,
) -> Option<u16> {
    let options = get_global(unsafe { PTR_PARSE_OPTIONS }); // Get global parse options
    if let Some(parsed_sql) = parse_select(sql, options) {
        let rs = gen_mapper(parsed_sql);
        if let Ok(mapper) = rs {
            let offsets = resolve_aggregate_offsets(&mapper, wrapped);
            let f = move |param: CallbackParams| call_aggregate(wrapped, offsets.as_slice(), param);
            register_mapper(mapper, FnHolder::Lambda(Box::new(f)))
        } else {
            log::warn!(
                "fail to create mapper from sql[{}], hit unexpected error: {:?}",
                sql,
                rs.err().unwrap()
            );
            None
        }
    } else {
        log::warn!("not supported sql statement: [{sql}]");
        None
    }
}

/// Resolves each stream column id referenced by the aggregate to the byte offset of
/// the same-named field in the mapper's dense SELECT output.
/// Returns a vec indexed by stream column id (1-based); usize::MAX means not resolved.
fn resolve_aggregate_offsets(mapper: &Mapper, wrapped: &WrappedAggregate) -> Arc<Vec<usize>> {
    let mut offsets = vec![usize::MAX];
    if let Some(stream) = Record::get_record(wrapped.aggregate().stream_id()) {
        offsets = vec![usize::MAX; stream._columns().len() + 1];
        for (name, off) in mapper.select_fields() {
            if let Some(field_id) = stream.column_id(name) {
                offsets[*field_id as usize] = *off;
            }
        }
    } else {
        log::warn!("failed to find stream by id[{}]", wrapped.aggregate().stream_id());
    }
    Arc::new(offsets)
}

/// Registers a generated mapper into the record-keyed mapper list.
fn register_mapper(mapper: Mapper, func_holder: FnHolder) -> Option<u16> {
    let id = mapper.id();
    let record_id = mapper.record_id(); // key by the target record id
    let mapper_map = get_global_mut::<SimpleU16Map>(unsafe { PTR_MAPPER_MAP });
    match mapper_map.entry(record_id) {
        SimpleU16Entry::Exist(_) => {
            // another mapper for the same record: append to the list
            if let Some(list) = mapper_map.get_mut::<WrappedMapperList>(record_id) {
                list.mappers.push(WrappedMapper::new(mapper, func_holder));
                Some(id)
            } else {
                log::warn!("mapper list for record [{record_id}] not found");
                None
            }
        }
        SimpleU16Entry::NotExist(_) => {
            mapper_map.insert(
                record_id,
                WrappedMapperList {
                    mappers: vec![WrappedMapper::new(mapper, func_holder)],
                },
            );
            Some(id)
        }
    }
}

/// Calls all mappers registered for the specified record ID by looking up the mapper list
/// and record, then invoking each mapper with the provided array and parameters.
#[inline]
pub(crate) fn call_mapper(array: &WrappedArray, id: u16) {
    let opt_mappers = search_mapper(id); // Look up the mapper list by record ID
    let opt_record = Record::get_record(id); // Look up the record by ID
    if opt_mappers.is_none() || opt_record.is_none() {
        return; // Return early if either mapper or record is not found
    }
    let mapper_list = opt_mappers.unwrap(); // Get the wrapped mapper list
    let record = opt_record.unwrap(); // Get the record
    for wrapped_mapper in &mapper_list.mappers {
        // Invoke each mapper with the array, mapper structure, record, and function holder
        invoke(
            id,
            array,
            &wrapped_mapper.mapper,
            record,
            &wrapped_mapper.fn_holder,
        );
    }
}

fn search_mapper(id: u16) -> Option<&'static WrappedMapperList> {
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
    let select_fields = build_select_fields(&parsed_sql);
    Ok(Mapper::new(
        id,
        parsed_sql,
        Executors::new(vec_executors.as_slice()),
        select_fields,
    ))
}

/// Builds the (field name, dense output offset) pairs for the mapper's SELECT fields.
/// The dense offsets match how Mapper::fetch packs fields into the callback buffer.
fn build_select_fields(parsed_sql: &ParsedSql) -> Vec<(String, usize)> {
    let mut fields = vec![];
    let mut offset = 0_usize;
    for entity in parsed_sql.fields() {
        let name = match entity {
            ExprEntity::Field(field_id) => Record::get_record(parsed_sql.records()[0])
                .map(|r| r.column(*field_id)._name().to_string())
                .unwrap_or_default(),
            ExprEntity::FieldWithTab(record_id, field_id) => Record::get_record(*record_id)
                .map(|r| r.column(*field_id)._name().to_string())
                .unwrap_or_default(),
            _ => String::new(),
        };
        fields.push((name, offset));
        offset += FIELD_SIZE;
    }
    fields
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
    let mask = array.mask(); // Get the array mask
    let first_idx = array.first_idx(); // Oldest write index still in window
    let last_idx = array.last_idx(); // Newest write index
    let step = array.step(); // Get the step size between elements
    let v_ptr = array.u64ptr(); // Get the array's u64 pointer
    let mut n = 0; // Counter for processed elements
    let mut offset = 0; // Offset in output buffer
    let limit = mapper.limit(); // Get the limit on number of results
    let asc = mapper.fetch_asc(); // Check if fetching in ascending order
                                  // Create an index wrapper that iterates in the appropriate direction
    let mut idx_wrapper = if asc {
        Idx::new(last_idx, first_idx, OPERATOR_DEC) // Newest to oldest
    } else {
        Idx::new(first_idx, last_idx, OPERATOR_INC) // Oldest to newest
    };
    let filter = mapper.filter(); // Get the filter function
    loop {
        // Calculate the slot position for this write index (mask wraps around)
        let position = (idx_wrapper.idx() * step) & mask;
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
    select_fields: Vec<(String, usize)>,
}
impl Mapper {
    fn new(
        id: u16,
        parsed_sql: ParsedSql,
        executors: Executors,
        select_fields: Vec<(String, usize)>,
    ) -> Mapper {
        Self {
            id,
            parsed_sql,
            executors,
            select_fields,
        }
    }

    pub(crate) fn id(&self) -> u16 {
        self.id
    }

    /// The target record id this mapper filters on.
    pub(crate) fn record_id(&self) -> u16 {
        self.parsed_sql.records()[0]
    }

    /// The (field name, dense output offset) pairs of the SELECT fields.
    pub(crate) fn select_fields(&self) -> &Vec<(String, usize)> {
        &self.select_fields
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

/// A list of mappers registered for the same record id.
pub(crate) struct WrappedMapperList {
    mappers: Vec<WrappedMapper>,
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
