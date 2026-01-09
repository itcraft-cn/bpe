/// Parameters passed to callback functions.
/// Contains pointers to data buffers and metadata for processing.
pub struct CallbackParams {
    u8_ptr: *const u8,
    mask: usize,
    offset: usize,
    size: usize,
    step: usize,
}
impl CallbackParams {
    pub fn new(
        u8_ptr: *const u8,
        mask: usize,
        offset: usize,
        size: usize,
        step: usize,
    ) -> Self {
        Self {
            u8_ptr,
            mask,
            offset,
            size,
            step,
        }
    }

    pub fn u8_ptr(&self) -> *const u8 {
        self.u8_ptr
    }

    pub fn mask(&self) -> usize {
        self.mask
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn step(&self) -> usize {
        self.step
    }
}