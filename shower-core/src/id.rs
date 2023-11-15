use std::sync::atomic::{AtomicU16, Ordering};

static mut RECORD_WALKER: Option<AtomicU16> = None;
static mut MAPPER_WALKER: Option<AtomicU16> = None;
static mut AGGREGATE_WALKER: Option<AtomicU16> = None;

pub(crate) fn init_walker() {
    unsafe {
        RECORD_WALKER = Some(AtomicU16::new(1));
        MAPPER_WALKER = Some(AtomicU16::new(1));
        AGGREGATE_WALKER = Some(AtomicU16::new(1));
    }
}

pub(crate) fn next_record_id() -> u16 {
    next_id(unsafe { RECORD_WALKER.as_ref().unwrap() })
}
pub(crate) fn next_mapper_id() -> u16 {
    next_id(unsafe { MAPPER_WALKER.as_ref().unwrap() })
}
pub(crate) fn next_aggregate_id() -> u16 {
    next_id(unsafe { AGGREGATE_WALKER.as_ref().unwrap() })
}
fn next_id(walker: &AtomicU16) -> u16 {
    walker.fetch_add(1, Ordering::SeqCst)
}
