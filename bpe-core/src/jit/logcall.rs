use crate::{cfg::get_config, consts::KEY_DEV_MODE, jit::base::GenContext};
use inkwell::values::IntValue;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Once,
};

pub(crate) fn _call_log<'ctx>(context: &GenContext<'ctx>, val: IntValue<'_>, msg: &str) {
    static DEV: AtomicBool = AtomicBool::new(false);
    static STOP: Once = Once::new();
    STOP.call_once(|| {
        DEV.store(get_config().fetch_cfg_bool(KEY_DEV_MODE), Ordering::SeqCst);
    });
    if DEV.load(Ordering::SeqCst) {
        _actual_call_log(context, val, msg);
    }
}

fn _actual_call_log<'ctx>(context: &GenContext<'ctx>, val: IntValue<'_>, msg: &str) {
    let log_func = context._log_func;
    let desc_msg_ptr = msg.as_ptr();
    let i64_type = context.func_generator.context.i64_type();
    let desc_msg_ptr_val = i64_type.const_int(desc_msg_ptr as u64, false);
    let desc_len = i64_type.const_int(msg.len() as u64, false);
    let _ = context
        .func_generator
        .builder
        .build_call(
            log_func,
            &[val.into(), desc_msg_ptr_val.into(), desc_len.into()],
            "log",
        )
        .unwrap();
}
