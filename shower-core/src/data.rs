#[derive(Clone, Copy, Debug)]
pub struct Tick {
    pub quote_id: u16,
    pub bid: u64,
    pub ask: u64,
    pub last: u64,
    pub volume: u64,
    pub timestamp: u64,
}
impl Tick {
    pub fn new(quote_id: u16, bid: u64, ask: u64, last: u64, volume: u64, timestamp: u64) -> Tick {
        Tick {
            quote_id,
            bid,
            ask,
            last,
            volume,
            timestamp,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DeepTick {
    pub quote_id: u16,
    pub depth: u8,
    pub bid: [u64; 20],
    pub ask: [u64; 20],
    pub last: u64,
    pub timestamp: u64,
}
impl DeepTick {
    pub fn new(
        quote_id: u16,
        depth: u8,
        bid: [u64; 20],
        ask: [u64; 20],
        last: u64,
        timestamp: u64,
    ) -> DeepTick {
        DeepTick {
            quote_id,
            depth,
            bid,
            ask,
            last,
            timestamp,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum BarType {
    BarMin,
    Bar5Min,
    Bar15Min,
    Bar30Min,
    Bar60Min,
    BarDaily,
    BarWeekly,
    BarMonthly,
    BarYearly,
}
#[derive(Clone, Copy, Debug)]
pub struct Bar {
    pub quote_id: u16,
    pub bar_type: BarType,
    pub open: u64,
    pub high: u64,
    pub low: u64,
    pub close: u64,
    pub volume: u64,
    pub timestamp: u64,
}
