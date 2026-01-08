use crate::{cfg::get_config, consts::KEY_DEV_MODE, jit::base::GenContext};
use inkwell::values::{BasicValue, BasicValueEnum};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Once,
};

static _DEV: AtomicBool = AtomicBool::new(false); // Static flag for development mode
static _FETCH_DEV_MODE: Once = Once::new(); // Once for one-time initialization

fn _init() {
    _FETCH_DEV_MODE.call_once(|| {
        // Initialize the DEV flag by reading the configuration once
        _DEV.store(get_config().fetch_cfg_bool(KEY_DEV_MODE), Ordering::SeqCst);
    });
}

/// Conditionally logs a value from JIT-compiled code if development mode is enabled.
/// This function checks the configuration to determine if logging should be performed.
pub(crate) fn _call_log<'ctx>(context: &GenContext<'ctx>, val: &dyn BasicValue<'_>, msg: &str) {
    _init();
    if _DEV.load(Ordering::SeqCst) {
        // Only call the actual log function if development mode is enabled
        _actual_call_log(context, val, msg);
    }
}

/// Actually performs the call to the logging function in the JIT environment.
/// Builds an LLVM call instruction to invoke the logging function with the specified parameters.
fn _actual_call_log<'ctx>(context: &GenContext<'ctx>, val: &dyn BasicValue<'_>, msg: &str) {
    let desc_msg_ptr = msg.as_ptr(); // Get pointer to the message string
    let i64_type = context.func_generator.context.i64_type(); // Get i64 type for LLVM

    // Create LLVM constant for the message pointer
    let desc_msg_ptr_val = i64_type.const_int(desc_msg_ptr as u64, false);
    // Create LLVM constant for the message length
    let desc_len = i64_type.const_int(msg.len() as u64, false);

    let val_enum = val.as_basic_value_enum();
    let rs = match val_enum {
        BasicValueEnum::IntValue(v) => Ok((
            context._log_int_func, // Get the logging function from context
            v.into(),              // Parameter: value
        )),
        BasicValueEnum::FloatValue(v) => Ok((
            context._log_float_func, // Get the logging function from context
            v.into(),                // Parameter: value
        )),
        _ => {
            log::warn!("Invalid value type for logging: {val_enum:?}");
            Err("Invalid value type for logging")
        }
    };
    if let Ok((func, val_meta)) = rs {
        let _ = context
            .func_generator
            .builder
            .build_call(
                func,                                                  // The logging function to call
                &[val_meta, desc_msg_ptr_val.into(), desc_len.into()], // Parameters: value, message pointer, length
                "log", // Name for the call instruction
            )
            .unwrap();
    }
}
