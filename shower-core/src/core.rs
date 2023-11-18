use crate::{
    aggregate::{call_aggregate, define_aggregate, init_aggregate_store, search_aggregate},
    cfg::load_config,
    data::{check_id, init_record_store, Column, Record, RecordType, U8Bytes},
    ffi::FfiFunc,
    func::FnHolder,
    id::init_walker,
    logger::init_logger,
    mapper::{call_mapper, define_mapper, init_mapper_store},
    store::{find_or_insert_array, init_store, insert, WrappedArray},
};
use std::sync::Once;

pub fn start() -> bool {
    static START: Once = Once::new();
    let mut opt = None;
    START.call_once(|| {
        opt.replace(actual_start());
    });
    if opt.is_none() {
        log::debug!("already started, skipping");
        true
    } else {
        opt.unwrap_or(false)
    }
}

fn actual_start() -> bool {
    load_config();
    init_logger();
    init_walker();
    init_mapper_store();
    init_aggregate_store();
    init_record_store();
    init_store();
    true
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
    if check_id(id) {
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
    call_mapper(array, data);
}

pub fn def_mapper<F>(sql: &str, func: F) -> Option<u16>
where
    F: Fn(Vec<[u8; 512]>) + Send + 'static,
{
    define_mapper(sql, FnHolder::Func(Box::new(func)))
}

pub fn def_mapper_bind_aggregate(sql: &str, aggregate_id: u16) -> Option<u16> {
    let opt_aggregate = search_aggregate(aggregate_id);
    if let Some(wrapped) = opt_aggregate {
        let f = move |data| call_aggregate(wrapped, data);
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
    F: Fn(Vec<[u8; 512]>) + Send + 'static,
{
    define_aggregate(sql, FnHolder::Func(Box::new(func)))
}

pub fn def_aggregate_ffi(sql: &str, ffi: Box<dyn FfiFunc>) -> Option<u16> {
    define_aggregate(sql, FnHolder::FfiFunc(ffi))
}
