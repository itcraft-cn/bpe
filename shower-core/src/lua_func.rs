use crate::U8Bytes;

pub(crate) fn id(v_ptr: u64) -> u16 {
    let ptr = v_ptr as *const &U8Bytes;
    let data = unsafe { *ptr };
    data.id()
}

pub(crate) fn mix(
    c0: u8,
    c1: u8,
    c2: u8,
    c3: u8,
    c4: u8,
    c5: u8,
    c6: u8,
    c7: u8,
    c8: u8,
    c9: u8,
    c10: u8,
    c11: u8,
    c12: u8,
    c13: u8,
    c14: u8,
    c15: u8,
) -> u128 {
    let mut mixed = 0u128;
    mixed += c0 as u128;
    mixed += (c1 as u128) << 2 * 8;
    mixed += (c2 as u128) << 3 * 8;
    mixed += (c3 as u128) << 4 * 8;
    mixed += (c4 as u128) << 5 * 8;
    mixed += (c5 as u128) << 6 * 8;
    mixed += (c6 as u128) << 7 * 8;
    mixed += (c7 as u128) << 8 * 8;
    mixed += (c8 as u128) << 8 * 8;
    mixed += (c9 as u128) << 9 * 8;
    mixed += (c10 as u128) << 10 * 8;
    mixed += (c11 as u128) << 11 * 8;
    mixed += (c12 as u128) << 12 * 8;
    mixed += (c13 as u128) << 13 * 8;
    mixed += (c14 as u128) << 14 * 8;
    mixed += (c15 as u128) << 15 * 8;
    mixed
}
