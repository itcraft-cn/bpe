use std::sync::atomic::{AtomicU16, Ordering};

static RECORD_WALKER: AtomicU16 = AtomicU16::new(1);
static MAPPER_WALKER: AtomicU16 = AtomicU16::new(1);
static AGGREGATE_WALKER: AtomicU16 = AtomicU16::new(1);
static WINDOW_WALKER: AtomicU16 = AtomicU16::new(1);

enum Walker {
    Record,
    Mapper,
    Aggregate,
    Window,
}

pub(crate) fn next_record_id() -> u16 {
    next_id(Walker::Record)
}
pub(crate) fn next_mapper_id() -> u16 {
    next_id(Walker::Mapper)
}
pub(crate) fn next_aggregate_id() -> u16 {
    next_id(Walker::Aggregate)
}
pub(crate) fn next_window_id() -> u16 {
    next_id(Walker::Window)
}
fn next_id(walker: Walker) -> u16 {
    match walker {
        Walker::Record => RECORD_WALKER.fetch_add(1, Ordering::SeqCst),
        Walker::Mapper => MAPPER_WALKER.fetch_add(1, Ordering::SeqCst),
        Walker::Aggregate => AGGREGATE_WALKER.fetch_add(1, Ordering::SeqCst),
        Walker::Window => WINDOW_WALKER.fetch_add(1, Ordering::SeqCst),
    }
}
