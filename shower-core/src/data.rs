#[derive(Clone)]
pub struct QuoteData {
    pub id: u16,
    pub bid: u128,
    pub ask: u128,
    pub last: u128,
    pub volume: u128,
    pub timestamp: u64,
}
impl QuoteData {
    pub fn new(
        id: u16,
        bid: u128,
        ask: u128,
        last: u128,
        volume: u128,
        timestamp: u64,
    ) -> QuoteData {
        QuoteData {
            id,
            bid,
            ask,
            last,
            volume,
            timestamp,
        }
    }
}
impl Copy for QuoteData {}
