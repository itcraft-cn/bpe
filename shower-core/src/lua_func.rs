use crate::data::U8Bytes;
use mlua::{Function, Lua};
use std::sync::Once;

pub(crate) fn inject_lua_func(lua_vm: &mut Lua) {
    static LUA_FUNC_INIT: Once = Once::new();
    LUA_FUNC_INIT.call_once(|| {
        let lua_def_func: Function = lua_vm.create_function(|_vm, ()| Ok(())).unwrap();
        let func1 = lua_vm
            .create_function(|_vm, v_ptr: u64| {
                let ptr = v_ptr as *const &U8Bytes;
                let data = unsafe { *ptr };
                Ok(data.id())
            })
            .unwrap_or_else(|_e| lua_def_func);
        let rs = lua_vm.globals().set("_id", func1);
        if let Err(e) = rs {
            log::info!("hit error: {}", e);
        }
    });
}
