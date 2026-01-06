use crate::{cfg::get_config, consts::KEY_DEV_MODE, jit::base::GenContext};
use inkwell::values::IntValue;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Once,
};

/// Conditionally logs a value from JIT-compiled code if development mode is enabled.
/// This function checks the configuration to determine if logging should be performed.
pub(crate) fn _call_log<'ctx>(context: &GenContext<'ctx>, val: IntValue<'_>, msg: &str) {
    static DEV: AtomicBool = AtomicBool::new(false);  // Static flag for development mode
    static STOP: Once = Once::new();                  // Once for one-time initialization
    STOP.call_once(|| {
        // Initialize the DEV flag by reading the configuration once
        DEV.store(get_config().fetch_cfg_bool(KEY_DEV_MODE), Ordering::SeqCst);
    });
    if DEV.load(Ordering::SeqCst) {
        // Only call the actual log function if development mode is enabled
        _actual_call_log(context, val, msg);
    }
}

/// Actually performs the call to the logging function in the JIT environment.
/// Builds an LLVM call instruction to invoke the logging function with the specified parameters.
fn _actual_call_log<'ctx>(context: &GenContext<'ctx>, val: IntValue<'_>, msg: &str) {
    let log_func = context._log_func;                // Get the logging function from context
    let desc_msg_ptr = msg.as_ptr();                 // Get pointer to the message string
    let i64_type = context.func_generator.context.i64_type();  // Get i64 type for LLVM
    // Create LLVM constant for the message pointer
    let desc_msg_ptr_val = i64_type.const_int(desc_msg_ptr as u64, false);
    // Create LLVM constant for the message length
    let desc_len = i64_type.const_int(msg.len() as u64, false);
    let _ = context
        .func_generator
        .builder
        .build_call(
            log_func,                                 // The logging function to call
            &[val.into(), desc_msg_ptr_val.into(), desc_len.into()],  // Parameters: value, message pointer, length
            "log",                                   // Name for the call instruction
        )
        .unwrap();
}