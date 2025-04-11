pub trait FfiFunc {
    fn callback(&self, data_ptr: *const u8, size: usize);
}
