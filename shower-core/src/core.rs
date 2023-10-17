use crate::{
    cfg::{get_config, load_config},
    consts::KEY_DEV_MODE,
    data::U8Bytes,
    logger::init_logger,
    sql::{parse_options, parse_sql},
    store,
};
use log::*;
use sql_parse::ParseOptions;
use std::sync::Once;

static mut DEBUG: bool = false;
static mut PARSE_OPTIONS: Option<ParseOptions> = None;

pub fn start() -> bool {
    static START: Once = Once::new();
    let mut opt = None;
    START.call_once(|| {
        opt.replace(actual_start());
    });
    if opt.is_none() {
        debug!("already started, skipping");
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
        PARSE_OPTIONS = Some(parse_options());
    }

    true
}

#[inline]
fn process_data(data: &U8Bytes) {
    store::insert(data);
    call_action(data);
}

#[inline]
fn call_action(_data: &U8Bytes) {}

pub fn stop() {
    static STOP: Once = Once::new();
    STOP.call_once(actual_stop);
}

fn actual_stop() {
    info!("mark as deactived");
}

pub fn new_data(data: &U8Bytes) -> bool {
    process_data(data);
    true
}

pub fn def_action(sql: &str) -> bool {
    info!("{}", sql);
    if let Some(parsed_sql) = parse_sql(sql, unsafe { PARSE_OPTIONS.as_ref().unwrap() }) {
        info!("can be used as a select statement: [{}]", sql);
        for table in &parsed_sql.tables() {
            info!("table: {}", table);
        }
        for filter in &parsed_sql.filters() {
            info!("filter: {:?}", filter);
        }
        for field in &parsed_sql.fields() {
            info!("field: {:?}", field);
        }
        true
    } else {
        warn!("not supported sql statement: [{}]", sql);
        false
    }
}
