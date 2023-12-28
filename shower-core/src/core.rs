use crate::{
    aggregate::{call_aggregate, define_aggregate, init_aggregate_store, search_aggregate},
    cfg::load_config,
    data::{check_id_in_store, init_record_store, Column, Record, RecordType, U8Bytes},
    ffi::FfiFunc,
    func::FnHolder,
    id::init_walker,
    jit::init_func_generator,
    logger::init_logger,
    mapper::{call_mapper, define_mapper, init_mapper_store},
    store::{find_or_insert_array, init_store, insert, WrappedArray},
};
use std::sync::Once;

pub fn start() {
    static START: Once = Once::new();
    START.call_once(actual_start);
}

fn actual_start() {
    load_config();
    init_logger();
    init_walker();
    init_func_generator();
    init_mapper_store();
    init_aggregate_store();
    init_record_store();
    init_store();
}

pub fn stop() {
    static STOP: Once = Once::new();
    STOP.call_once(actual_stop);
}

fn actual_stop() {
    log::info!("mark as deactived");
}

pub fn def_incoming(name: &str, columns: Vec<Column>) -> Option<u16> {
    Record::insert_record(name, RecordType::Incoming, columns)
}

pub fn def_stream(name: &str, columns: Vec<Column>) -> Option<u16> {
    Record::insert_record(name, RecordType::Stream, columns)
}

pub fn new_data(data: &U8Bytes) -> bool {
    let id = data.id();
    if check_id_in_store(id) {
        log::warn!("id [{}] is not defined", id);
        false
    } else {
        let array = find_or_insert_array(id);
        process_data(array, data);
        true
    }
}

#[inline]
fn process_data(array: &mut WrappedArray, data: &U8Bytes) {
    insert(array, data);
    call_mapper(array, data.id());
}

pub fn def_mapper<F>(sql: &str, func: F) -> Option<u16>
where
    F: Fn(*const u8, usize) + Send + 'static,
{
    define_mapper(sql, FnHolder::Func(Box::new(func)))
}

pub fn def_mapper_bind_aggregate(sql: &str, aggregate_id: u16) -> Option<u16> {
    let opt_aggregate = search_aggregate(aggregate_id);
    if let Some(wrapped) = opt_aggregate {
        let f = move |u8_ptr: *const u8, size: usize| call_aggregate(wrapped, u8_ptr, size);
        define_mapper(sql, FnHolder::Lambda(Box::new(f)))
    } else {
        None
    }
}

pub fn def_mapper_ffi(sql: &str, ffi: Box<dyn FfiFunc>) -> Option<u16> {
    define_mapper(sql, FnHolder::FfiFunc(ffi))
}

pub fn def_aggregate<F>(sql: &str, func: F) -> Option<u16>
where
    F: Fn(*const u8, usize) + Send + 'static,
{
    define_aggregate(sql, FnHolder::Func(Box::new(func)))
}

pub fn def_aggregate_ffi(sql: &str, ffi: Box<dyn FfiFunc>) -> Option<u16> {
    define_aggregate(sql, FnHolder::FfiFunc(ffi))
}
