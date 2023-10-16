use crate::lua_func::{id, mix};
use mlua::{Error, FromLuaMulti, Function, Lua, ToLuaMulti};
use std::sync::Once;

pub(crate) fn inject_lua_func(lua_vm: &Lua) {
    static LUA_FUNC_INIT: Once = Once::new();
    LUA_FUNC_INIT.call_once(|| {
        reg_func(lua_vm, "_id", |_, v_ptr: u64| Ok(id(v_ptr)));
        reg_func(
            lua_vm,
            "_mix_array_to_u128",
            |_,
             (c0, c1, c2, c3, c4, c5, c6, c7, c8, c9, c10, c11, c12, c13, c14, c15): (
                u8,
                u8,
                u8,
                u8,
                u8,
                u8,
                u8,
                u8,
                u8,
                u8,
                u8,
                u8,
                u8,
                u8,
                u8,
                u8,
            )| {
                Ok(mix(
                    c0, c1, c2, c3, c4, c5, c6, c7, c8, c9, c10, c11, c12, c13, c14, c15,
                ))
            },
        );
        reg_func(
            lua_vm,
            "_select",
            |_, (_v_ptr, _mixed_id, _mixed_func): (u64, u128, u128)| Ok(()),
        );
    });
}

fn reg_func<'a, F, A, R>(lua_vm: &'a Lua, key: &'a str, closure: F)
where
    F: 'static + Fn(&Lua, A) -> Result<R, Error>,
    A: FromLuaMulti<'a>,
    R: ToLuaMulti<'a>,
{
    let rs = lua_vm.globals().set(key, new_func(lua_vm, closure));
    if let Err(e) = rs {
        log::info!("hit error: {}", e);
    }
}

fn new_func<'a, F, A, R>(lua_vm: &'a Lua, closure: F) -> Function<'a>
where
    F: 'static + Fn(&Lua, A) -> Result<R, Error>,
    A: FromLuaMulti<'a>,
    R: ToLuaMulti<'a>,
{
    lua_vm.create_function(closure).unwrap_or_else(|_e| {
        let lua_def_func: Function = lua_vm.create_function(|_vm, ()| Ok(())).unwrap();
        lua_def_func
    })
}
