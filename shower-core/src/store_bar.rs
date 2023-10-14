use crate::data::{Bar, BarType, U8Tick};

pub(crate) fn update_tick(_u8tick: &U8Tick, _idx: usize) {}

struct _MixedBarStore {
    bar_array: [_BarStore; 8],
}
impl _MixedBarStore {
    fn _new() -> Self {
        _MixedBarStore {
            bar_array: [
                _BarStore::_new(BarType::BarMin),
                _BarStore::_new(BarType::Bar5Min),
                _BarStore::_new(BarType::Bar15Min),
                _BarStore::_new(BarType::Bar30Min),
                _BarStore::_new(BarType::BarHour),
                _BarStore::_new(BarType::BarDaily),
                _BarStore::_new(BarType::BarWeekly),
                _BarStore::_new(BarType::BarMonthly),
            ],
        }
    }
}

struct _BarStore {
    bar_type: BarType,
    data: Vec<u8>,
    current_bar: Option<Bar>,
}
impl _BarStore {
    fn _new(bar_type: BarType) -> Self {
        _BarStore {
            bar_type,
            data: vec![0u8; 1024],
            current_bar: None,
        }
    }
}
