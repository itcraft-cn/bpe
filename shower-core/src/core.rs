use crate::{
    cfg::{get_config, load_config},
    consts::KEY_DEV_MODE,
    data::U8Bytes,
    logger::init_logger,
    lua::inject_lua_func,
    store,
};
use log::*;
use mlua::{Function, Lua};
use std::{ptr, sync::Once};

static mut DEBUG: bool = false;
static mut LUA_VM: Option<Lua> = None;
static mut ACTION_VEC: Option<Vec<Function>> = None;

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
        let mut lua_vm = Lua::new();
        inject_lua_func(&mut lua_vm);
        LUA_VM = Some(lua_vm);
        ACTION_VEC = Some(vec![]);
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
    let vec = unsafe { ACTION_VEC.as_ref().unwrap() };
    if !vec.is_empty() {
        let ptr = ptr::addr_of!(data);
        let v_ptr = ptr as u64;
        vec.iter().for_each(|f| {
            call_lua(f, v_ptr);
        });
    }
}

#[inline]
fn call_lua(f: &Function<'_>, v_ptr: u64) {
    let rs = f.call::<u64, ()>(v_ptr);
    if let Err(e) = rs {
        warn!("hit error: {}", e);
    }
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

pub fn def_action_sql(sql: &str) -> bool {
    info!("{}", sql);
    todo!("not implemented yet");
}

pub fn def_action_lua(lua: &str) -> bool {
    info!("{}", lua);
    let lua_vm = unsafe {
        LUA_VM
            .as_mut()
            .unwrap_or_else(|| panic!("should have a valid lua vm"))
    };
    let rs: Result<Function, mlua::Error> = lua_vm.load(lua).eval();
    if let Ok(lua_func) = rs {
        let vec = unsafe {
            ACTION_VEC
                .as_mut()
                .unwrap_or_else(|| panic!("should have a valid action map"))
        };
        vec.push(lua_func);
        true
    } else {
        false
    }
}
