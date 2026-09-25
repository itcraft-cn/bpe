#[macro_export]
macro_rules! log_val {
    ($v:expr, $d:expr, $l:expr) => {
        let ptr = $d as *const u8; // Convert description pointer to byte pointer
        if let Ok(msg) = pointer_to_str_safe(ptr, $l as usize) {
            log::info!("|jit|[{}]=>[{}]", msg, $v); // Log the message with data value
        } else {
            log::warn!("cannot convert to string"); // Log warning if string conversion fails
        }
    };
}
