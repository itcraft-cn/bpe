pub trait FfiFunc {
    fn callback(&self, data: Vec<[u8; 512]>);
}
