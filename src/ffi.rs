use crate::param::CallbackParams;

/// Trait for FFI (Foreign Function Interface) callback functions.
/// This allows external functions (potentially from other languages) to be called
/// as mapper or aggregate callbacks.
pub trait FfiFunc {
    fn callback(&self, param: CallbackParams);
}
