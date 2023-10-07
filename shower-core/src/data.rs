#[derive(Clone, Copy, Debug)]
pub struct Tick {
    pub quote_id: u16,
    pub bid: u128,
    pub ask: u128,
    pub last: u128,
    pub volume: u128,
    pub timestamp: u64,
}
impl Tick {
    pub fn new(
        quote_id: u16,
        bid: u128,
        ask: u128,
        last: u128,
        volume: u128,
        timestamp: u64,
    ) -> Tick {
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
    pub bid: [u128; 20],
    pub ask: [u128; 20],
    pub last: u128,
    pub timestamp: u64,
}
impl DeepTick {
    pub fn new(
        quote_id: u16,
        depth: u8,
        bid: [u128; 20],
        ask: [u128; 20],
        last: u128,
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
    pub open: u128,
    pub high: u128,
    pub low: u128,
    pub close: u128,
    pub volume: u128,
}
