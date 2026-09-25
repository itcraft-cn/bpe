/// Parameters passed to callback functions.
/// Contains pointers to data buffers and metadata for processing.
pub struct CallbackParams {
    u8_ptr: *const u8,
    mask: usize,
    offset: usize,
    size: usize,
    step: usize,
    /// Window start time (ms, event time) when triggered by a window aggregate; 0 otherwise.
    win_start_ms: i64,
    /// Window end time (ms, event time) when triggered by a window aggregate; 0 otherwise.
    win_end_ms: i64,
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
            win_start_ms: 0,
            win_end_ms: 0,
        }
    }

    /// Creates parameters carrying the triggering window's time range.
    pub(crate) fn new_with_window(
        u8_ptr: *const u8,
        mask: usize,
        offset: usize,
        size: usize,
        step: usize,
        win_start_ms: i64,
        win_end_ms: i64,
    ) -> Self {
        Self {
            u8_ptr,
            mask,
            offset,
            size,
            step,
            win_start_ms,
            win_end_ms,
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

    /// Start of the triggering window (ms, event time). 0 for non-window callbacks.
    pub fn window_start_ms(&self) -> i64 {
        self.win_start_ms
    }

    /// End of the triggering window (ms, event time). 0 for non-window callbacks.
    pub fn window_end_ms(&self) -> i64 {
        self.win_end_ms
    }
}
