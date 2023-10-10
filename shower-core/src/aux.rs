use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(crate) fn fill_u64(slice: &mut [u8], data: u64) {
    slice[0] = (data >> 0) as u8;
    slice[1] = (data >> 1) as u8;
    slice[2] = (data >> 2) as u8;
    slice[3] = (data >> 3) as u8;
    slice[4] = (data >> 4) as u8;
    slice[5] = (data >> 5) as u8;
    slice[6] = (data >> 6) as u8;
    slice[7] = (data >> 7) as u8;
}

pub(crate) fn _fetch_u64(slice: &[u8]) -> u64 {
    ((slice[0] as u64) << 0)
        | ((slice[1] as u64) << 1)
        | ((slice[2] as u64) << 2)
        | ((slice[3] as u64) << 3)
        | ((slice[4] as u64) << 4)
        | ((slice[5] as u64) << 5)
        | ((slice[6] as u64) << 6)
        | ((slice[7] as u64) << 7)
}

fn _timestamp() -> u64 {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_e| Duration::new(0, 0));
    duration.as_millis() as u64
}
