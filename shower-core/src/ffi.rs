pub trait FfiFunc {
    fn callback(&self, data: Vec<[u64; 64]>);
}
