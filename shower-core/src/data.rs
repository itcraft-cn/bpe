use crate::consts::MAX_DEEP_TICK_DEPTH;

pub(crate) const TICK_ELEMENT_SIZE: usize = 5;
pub(crate) const TICK_SIZE: usize = 64;
pub(crate) const DEEP_TICK_ELEMENT_SIZE: usize = 63;
pub(crate) const DEEP_TICK_SIZE: usize = 512;

pub(crate) struct U8Tick {
    quote_id: u16,
    element_size: usize,
    tick_size: usize,
    u8_data_len: usize,
    data: [u64; 64],
}
impl U8Tick {
    pub fn quote_id(&self) -> u16 {
        self.quote_id
    }
    pub fn element_size(&self) -> usize {
        self.element_size
    }
    pub fn tick_size(&self) -> usize {
        self.tick_size
    }
    pub fn u8_tick_data_len(&self) -> usize {
        self.u8_data_len
    }
    pub fn u64data(&self) -> &[u64] {
        self.data.as_slice()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Tick {
    quote_id: u16,
    bid: u64,
    ask: u64,
    last: u64,
    volume: u64,
    timestamp: u64,
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

    pub(crate) fn convert(&self, u8_data_len: usize) -> U8Tick {
        U8Tick {
            quote_id: self.quote_id,
            element_size: TICK_ELEMENT_SIZE,
            tick_size: TICK_SIZE,
            u8_data_len,
            data: [
                self.bid,
                self.ask,
                self.last,
                self.volume,
                self.timestamp,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DeepTick {
    quote_id: u16,
    bid: [u64; MAX_DEEP_TICK_DEPTH],
    ask: [u64; MAX_DEEP_TICK_DEPTH],
    last: u64,
    volume: u64,
    timestamp: u64,
}
impl DeepTick {
    pub fn new(
        quote_id: u16,
        bid: [u64; MAX_DEEP_TICK_DEPTH],
        ask: [u64; MAX_DEEP_TICK_DEPTH],
        last: u64,
        volume: u64,
        timestamp: u64,
    ) -> DeepTick {
        DeepTick {
            quote_id,
            bid,
            ask,
            last,
            volume,
            timestamp,
        }
    }

    pub(crate) fn convert(&self, u8_data_len: usize) -> U8Tick {
        U8Tick {
            quote_id: self.quote_id,
            element_size: DEEP_TICK_ELEMENT_SIZE,
            tick_size: DEEP_TICK_SIZE,
            u8_data_len,
            data: [
                self.bid[0],
                self.bid[1],
                self.bid[2],
                self.bid[3],
                self.bid[4],
                self.bid[5],
                self.bid[6],
                self.bid[7],
                self.bid[8],
                self.bid[9],
                self.bid[10],
                self.bid[11],
                self.bid[12],
                self.bid[13],
                self.bid[14],
                self.bid[15],
                self.bid[16],
                self.bid[17],
                self.bid[18],
                self.bid[19],
                self.bid[20],
                self.bid[21],
                self.bid[22],
                self.bid[23],
                self.bid[24],
                self.bid[25],
                self.bid[26],
                self.bid[27],
                self.bid[28],
                self.bid[29],
                self.ask[0],
                self.ask[1],
                self.ask[2],
                self.ask[3],
                self.ask[4],
                self.ask[5],
                self.ask[6],
                self.ask[7],
                self.ask[8],
                self.ask[9],
                self.ask[10],
                self.ask[11],
                self.ask[12],
                self.ask[13],
                self.ask[14],
                self.ask[15],
                self.ask[16],
                self.ask[17],
                self.ask[18],
                self.ask[19],
                self.ask[20],
                self.ask[21],
                self.ask[22],
                self.ask[23],
                self.ask[24],
                self.ask[25],
                self.ask[26],
                self.ask[27],
                self.ask[28],
                self.ask[29],
                self.last,
                self.volume,
                self.timestamp,
                0,
            ],
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
