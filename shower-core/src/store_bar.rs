use crate::data::{Bar, BarType, U8Tick};

pub(crate) fn update_tick(_u8tick: &U8Tick, _idx: usize) {}

struct MixedBarStore {
    bar_array: [BarStore; 8],
}
impl MixedBarStore {
    fn new() -> Self {
        MixedBarStore {
            bar_array: [
                BarStore::new(BarType::BarMin),
                BarStore::new(BarType::Bar5Min),
                BarStore::new(BarType::Bar15Min),
                BarStore::new(BarType::Bar30Min),
                BarStore::new(BarType::BarHour),
                BarStore::new(BarType::BarDaily),
                BarStore::new(BarType::BarWeekly),
                BarStore::new(BarType::BarMonthly),
            ],
        }
    }
}

struct BarStore {
    bar_type: BarType,
    data: Vec<u8>,
    current_bar: Option<Bar>,
}
impl BarStore {
    fn new(bar_type: BarType) -> Self {
        BarStore {
            bar_type,
            data: vec![0u8; 1024],
            current_bar: None,
        }
    }
}
