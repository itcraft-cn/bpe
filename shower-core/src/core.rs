use crate::{
    aggregate::{call_aggregate, define_aggregate, init_aggregate_store, search_aggregate},
    cfg::{get_config, load_config},
    consts::KEY_DEV_MODE,
    data::{check_id, init_record_store, insert_record, Column, RecordType, U8Bytes},
    ffi::FfiFunc,
    func::FnHolder,
    logger::init_logger,
    mapper::{call_mapper, define_mapper, init_mapper_store},
    store::insert,
};
use std::sync::Once;

static mut DEBUG: bool = false;

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
    init_logger(get_config());

    let cfg = get_config();

    unsafe {
        DEBUG = cfg.fetch_cfg_bool(KEY_DEV_MODE);
        init_mapper_store();
        init_aggregate_store();
        init_record_store();
    }
    true
}

pub fn stop() {
    static STOP: Once = Once::new();
    STOP.call_once(actual_stop);
}

pub fn def_incoming(columns: Vec<Column>) -> u16 {
    insert_record(RecordType::Incoming, columns)
}

pub fn def_stream(columns: Vec<Column>) -> u16 {
    insert_record(RecordType::Stream, columns)
}

fn actual_stop() {
    log::info!("mark as deactived");
}

pub fn new_data(data: &U8Bytes) -> bool {
    let id = data.id();
    if check_id(id) {
        log::warn!("id [{}] is not defined", id);
        false
    } else {
        process_data(data);
        true
    }
}

#[inline]
fn process_data(data: &U8Bytes) {
    insert(data);
    call_mapper(data);
}

pub fn def_mapper<F>(sql: &str, func: F) -> bool
where
    F: Fn(Vec<[u8; 512]>) + Send + 'static,
{
    define_mapper(sql, FnHolder::Func(Box::new(func)))
}

pub fn def_mapper_with_aggregate(sql: &str, aggregate_id: u16) -> bool {
    let opt_aggregate = search_aggregate(aggregate_id);
    if let Some(_wrapped) = opt_aggregate {
        let f = move |_data| call_aggregate(aggregate_id, _wrapped, _data);
        define_mapper(sql, FnHolder::Lambda(Box::new(f)))
    } else {
        false
    }
}

pub fn def_mapper_ffi(sql: &str, ffi: Box<dyn FfiFunc>) -> bool {
    define_mapper(sql, FnHolder::FfiFunc(ffi))
}

pub fn def_aggregate<F>(sql: &str, func: F) -> bool
where
    F: Fn(Vec<[u8; 512]>) + Send + 'static,
{
    define_aggregate(sql, FnHolder::Func(Box::new(func)))
}

pub fn def_aggregate_ffi(sql: &str, ffi: Box<dyn FfiFunc>) -> bool {
    define_aggregate(sql, FnHolder::FfiFunc(ffi))
}
