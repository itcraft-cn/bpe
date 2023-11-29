pub trait FfiFunc {
    fn callback(&self, data: &[[u8; 512]]);
}
