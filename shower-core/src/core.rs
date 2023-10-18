use crate::{
    action::{gen_action, invoke, Action},
    aux::SimpleU16Map,
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
static mut ACTION_MAP: Option<SimpleU16Map<Vec<Action>>> = None;

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
        ACTION_MAP = Some(SimpleU16Map::new());
    }

    true
}

#[inline]
fn process_data(data: &U8Bytes) {
    store::insert(data);
    call_action(data);
}

#[inline]
fn call_action(data: &U8Bytes) {
    let actions = search_aciton(data.id());
    for action in actions {
        invoke(action);
    }
}

fn search_aciton<'a>(id: u16) -> &'a Vec<Action<'a>> {
    let map = unsafe { ACTION_MAP.as_ref().unwrap() };
    map.get(id)
}

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
    let opt_parsed_sql = parse_sql(sql, unsafe { PARSE_OPTIONS.as_ref().unwrap() });
    if let Some(parsed_sql) = opt_parsed_sql {
        let rs = gen_action(&parsed_sql);
        if let Ok(action) = rs {
            let id = action.id();
            let map = unsafe { ACTION_MAP.as_mut().unwrap() };
            map.entry(id).or_insert_with(map, Vec::new);
            map.get_mut(id).push(action);
            true
        } else {
            log::warn!(
                "fail to create action from sql[{}], hit unexpected error: {:?}",
                sql,
                rs.err().unwrap()
            );
            false
        }
    } else {
        warn!("not supported sql statement: [{}]", sql);
        false
    }
}
