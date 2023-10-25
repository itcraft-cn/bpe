use crate::{
    action::{gen_action, invoke, Action, FnHolder},
    aux::SimpleU16Map,
    cfg::{get_config, load_config},
    consts::KEY_DEV_MODE,
    data::U8Bytes,
    ffi::FfiFunc,
    logger::init_logger,
    sql::{parse_options, parse_sql},
    store::insert,
};
use log::*;
use sql_parse::ParseOptions;
use std::sync::Once;

static mut DEBUG: bool = false;
static mut PARSE_OPTIONS: Option<ParseOptions> = None;
static mut ACTION_MAP: Option<SimpleU16Map<Vec<(Action, FnHolder)>>> = None;

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

#[inline]
fn process_data(data: &U8Bytes) {
    insert(data);
    call_action(data);
}

#[inline]
fn call_action(data: &U8Bytes) {
    let opt_actions = search_aciton(data.id());
    if let Some(actions) = opt_actions {
        for (action, fn_holder) in actions {
            invoke(action, fn_holder);
        }
    }
}

fn search_aciton<'a>(id: u16) -> Option<&'a Vec<(Action<'a>, FnHolder)>> {
    let map = unsafe { ACTION_MAP.as_ref().unwrap() };
    map.get(id)
}

pub fn def_action(sql: &str) -> bool {
    actual_def_action(sql, FnHolder::NotExist)
}

pub fn def_action_with_callback<F>(sql: &str, func: F) -> bool
where
    F: Fn(Vec<[u64; 64]>) + Send + 'static,
{
    actual_def_action(sql, FnHolder::Func(Box::new(func)))
}

pub fn def_action_ffi(sql: &str, ffi: Box<dyn FfiFunc>) -> bool {
    actual_def_action(sql, FnHolder::FfiFunc(ffi))
}

fn actual_def_action(sql: &str, func_holder: FnHolder) -> bool {
    let opt_parsed_sql = parse_sql(sql, unsafe { PARSE_OPTIONS.as_ref().unwrap() });
    if let Some(parsed_sql) = opt_parsed_sql {
        let rs = gen_action(&parsed_sql);
        if let Ok(action) = rs {
            let id = action.id();
            let map = unsafe { ACTION_MAP.as_mut().unwrap() };
            map.entry(id).or_insert_with(map, Vec::new);
            map.get_mut(id).push((action, func_holder));
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
        log::warn!("not supported sql statement: [{}]", sql);
        false
    }
}
