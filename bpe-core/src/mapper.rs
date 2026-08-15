use crate::{
    aggregate::{call_aggregate, WrappedAggregate},
    aux::{SimpleU16Entry, SimpleU16Map},
    callback::{callback, FnHolder},
    consts::FIELD_SIZE,
    data::{Record, U8Bytes},
    error::ParseSqlError,
    exec::{create_executor, Executors},
    id::next_mapper_id,
    param::CallbackParams,
    sql::{
        base::{parse_options, ExprEntity, FilterFunc, ParsedSql},
        select::parse_select,
    },
    store::{find_or_insert_array, get_record_size, get_vec_size, WrappedArray},
};
use globalvar::{def_global_ptr, get_global, get_global_mut};
use inkwell::execution_engine::JitFunction;
use std::{alloc::{self, Layout}, sync::{atomic::{AtomicU64, Ordering}, Arc}};


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
//#[inline]
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

/// Updates the per-slot hit bitmap of every mapper registered for the record:
/// the JIT filter runs exactly once per new record (not once per window scan),
/// and the result is stored in the slot bitmap used by `loop_filter`.
///
/// Filters that depend on mutable dimension tables snapshot the dimension state
/// at insert time; when the dimension version changes, all bitmaps are rebuilt
/// on this (engine) thread before the new record is filtered.
pub(crate) fn update_hitmap(array: &WrappedArray, data: &U8Bytes) {
    let opt_mappers = search_mapper(data.id());
    let Some(mapper_list) = opt_mappers else { return };
    let ver = crate::dimension::dim_version();
    if ver != 0
        && mapper_list
            .mappers
            .iter()
            .any(|w| w.mapper.dim_version.load(Ordering::Relaxed) != ver)
    {
        refresh_all_hitmaps();
    }
    let slot = array.newest_slot();
    let v_ptr = data.bytes().as_ptr() as u64;
    for wrapped in &mapper_list.mappers {
        let hit = unsafe { wrapped.mapper.filter().call(v_ptr) };
        wrapped.mapper.set_hit(slot, hit);
        wrapped.mapper.dim_version.store(ver, Ordering::Relaxed);
    }
}

/// Rebuilds every mapper's hit bitmap by re-running its JIT filter over the
/// whole window of its record. Runs on the engine thread (from `update_hitmap`),
/// so it does not race with bitmap reads in `loop_filter`.
fn refresh_all_hitmaps() {
    let record_map = get_global::<SimpleU16Map>(crate::data::record_map_ptr());
    let ver = crate::dimension::dim_version();
    record_map.for_each_id(|record_id| {
        let opt_mappers = search_mapper(record_id);
        let Some(mapper_list) = opt_mappers else { return };
        let array = find_or_insert_array(record_id);
        let count = array.walker();
        let max_records = array.max_records();
        let first_w = if count > max_records { count - max_records } else { 0 };
        let step = array.step();
        for wrapped in &mapper_list.mappers {
            for w in first_w..count {
                let slot = w % max_records;
                let sub = array.sub_data(slot * step);
                let hit = unsafe { wrapped.mapper.filter().call(sub as u64) };
                wrapped.mapper.set_hit(slot, hit);
                wrapped.mapper.dim_version.store(ver, Ordering::Relaxed);
            }
        }
    });
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

/// Invokes a mapper by scanning the per-slot hit bitmap for the newest matching
/// records, then calling the callback function with the filtered results.
fn invoke(
    id: u16,                 // Record ID
    array: &WrappedArray,    // Array containing the data
    mapper: &'static Mapper, // The mapper to invoke
    record: &Record,         // The record definition
    fn_holder: &FnHolder,    // The function to call with results
) {
    // Get pointer to value data reference for processing
    let u8_ptr = unsafe { PTR_VAL_DATA_REF } as *mut u8;
    // Collect matching records by scanning the hit bitmap (filter already ran per record)
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
/// Scans the per-slot hit bitmap by 64-bit words (skipping zero words, locating
/// set bits via TZCNT/LZCNT), so a full window scan costs a handful of word tests
/// instead of one bit test per record.
fn loop_filter(
    array: &WrappedArray, // Array containing the data to filter
    mapper: &Mapper,      // The mapper with filter criteria
    id: u16,              // Record ID
    record: &Record,      // The record definition
    u8_ptr: *mut u8,      // Pointer to output buffer
) -> usize {
    let count = array.walker(); // number of records written
    let max_records = array.max_records();
    let first_w = if count > max_records { count - max_records } else { 0 }; // oldest write index
    let last_w = count - 1; // newest write index
    let win_len = count - first_w; // number of records in the window
    let num_words = (max_records / 64).max(1);
    let first_slot = first_w % max_records;
    let last_slot = last_w % max_records;
    let step = array.step();
    let v_ptr = array.u64ptr();
    let limit = mapper.limit();
    let asc = mapper.fetch_asc(); // newest-first when LIMIT >= 0
    let hitmap = mapper.hitmap();
    let mut n = 0; // matched records
    let mut offset = 0; // output buffer offset
    let mut scanned = 0usize; // slots examined

    let process = |slot: usize,
                   n: &mut usize,
                   offset: &mut usize,
                   u8_ptr: *mut u8| {
        let position = slot * step;
        let sub_data_ptr = array.sub_data(position);
        let adjusted = unsafe { u8_ptr.add(*offset) };
        mapper.fetch(id, v_ptr, record, position, sub_data_ptr, adjusted);
        *n += 1;
        *offset += step;
    };

    if asc {
        // newest -> oldest: walk words backwards from the newest slot
        let mut word_idx = last_slot / 64;
        let mut start_bit = last_slot % 64;
        while scanned < win_len && n < limit {
            let mask = if start_bit >= 63 {
                u64::MAX
            } else {
                (1u64 << (start_bit + 1)) - 1
            };
            let mut word = unsafe { *(hitmap.add(word_idx * 8) as *const u64) } & mask;
            scanned += start_bit + 1;
            while word != 0 && n < limit {
                let bit = 63 - word.leading_zeros() as usize;
                let slot = word_idx * 64 + bit;
                if slot < max_records {
                    process(slot, &mut n, &mut offset, u8_ptr);
                }
                word &= !(1 << bit);
            }
            word_idx = (word_idx + num_words - 1) % num_words;
            start_bit = 63;
        }
    } else {
        // oldest -> newest: walk words forward from the oldest slot
        let mut word_idx = first_slot / 64;
        let mut start_bit = first_slot % 64;
        while scanned < win_len && n < limit {
            let mask = if start_bit == 0 {
                u64::MAX
            } else {
                u64::MAX << start_bit
            };
            let mut word = unsafe { *(hitmap.add(word_idx * 8) as *const u64) } & mask;
            scanned += 64 - start_bit;
            while word != 0 && n < limit {
                let bit = word.trailing_zeros() as usize;
                let slot = word_idx * 64 + bit;
                if slot < max_records {
                    process(slot, &mut n, &mut offset, u8_ptr);
                }
                word &= word - 1;
            }
            word_idx = (word_idx + 1) % num_words;
            start_bit = 0;
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
    /// Per-slot hit bitmap (1 bit per ring-buffer slot): the JIT filter result
    /// for the record currently stored in each slot. Set once per record at
    /// insert time by `update_hitmap`; scanned by `loop_filter`.
    /// Raw pointer: accessed from the single engine thread (insert + scan).
    hitmap: *mut u8,
    /// Dimension version when the bitmap was last built (stale detection).
    dim_version: AtomicU64,
}
impl Mapper {
    fn new(
        id: u16,
        parsed_sql: ParsedSql,
        executors: Executors,
        select_fields: Vec<(String, usize)>,
    ) -> Mapper {
        // one bit per ring-buffer slot
        let hitmap_len = get_vec_size() / get_record_size() / 8 + 1;
        let hitmap = unsafe {
            std::alloc::alloc_zeroed(std::alloc::Layout::from_size_align(hitmap_len, 1).unwrap())
        };
        Self {
            id,
            parsed_sql,
            executors,
            select_fields,
            hitmap,
            dim_version: AtomicU64::new(0),
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

    /// Records the JIT filter result for the record stored in `slot`.
    /// Single-threaded access: called from the engine's insert path.
    fn set_hit(&self, slot: usize, hit: bool) {
        unsafe {
            let p = self.hitmap.add(slot >> 3);
            let bit = 1 << (slot & 7);
            if hit {
                *p |= bit;
            } else {
                *p &= !bit;
            }
        }
    }

    fn hitmap(&self) -> *mut u8 {
        self.hitmap
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
