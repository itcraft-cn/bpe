use crate::{aux::_fetch_u64, consts::DEFALUT_SELECT_SIZE, store, U8Bytes};

pub(crate) fn rust_lua_fn_id(v_ptr: u64) -> u16 {
    let ptr = v_ptr as *const &U8Bytes;
    let data = unsafe { *ptr };
    data.id()
}

pub(crate) fn rust_lua_fn_mix(
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
    let mut mixed = c0 as u128;
    mixed += (c1 as u128) << (2 * 8);
    mixed += (c2 as u128) << (3 * 8);
    mixed += (c3 as u128) << (4 * 8);
    mixed += (c4 as u128) << (5 * 8);
    mixed += (c5 as u128) << (6 * 8);
    mixed += (c6 as u128) << (7 * 8);
    mixed += (c7 as u128) << (8 * 8);
    mixed += (c8 as u128) << (8 * 8);
    mixed += (c9 as u128) << (9 * 8);
    mixed += (c10 as u128) << (10 * 8);
    mixed += (c11 as u128) << (11 * 8);
    mixed += (c12 as u128) << (12 * 8);
    mixed += (c13 as u128) << (13 * 8);
    mixed += (c14 as u128) << (14 * 8);
    mixed += (c15 as u128) << (15 * 8);
    mixed
}

pub(crate) fn rust_lua_fn_select(id: u16, mixed_id: u128, mixed_func: u128, len: u8) {
    let mut id_array = [0u8; 16];
    fill(&mut id_array, mixed_id, len);
    let mut fn_array = [0u8; 16];
    fill(&mut fn_array, mixed_func, len);
    select(id, &id_array, &fn_array, len);
}

fn fill(array: &mut [u8; 16], mixed: u128, len: u8) {
    for i in 0..len {
        array[i as usize] = (mixed >> (i * 8)) as u8;
    }
}

fn select(id: u16, id_array: &[u8; 16], fn_array: &[u8; 16], len: u8) {
    select_limit(id, id_array, fn_array, len, DEFALUT_SELECT_SIZE);
}

fn select_limit(id: u16, _id_array: &[u8; 16], _fn_array: &[u8; 16], _len: u8, limit: usize) {
    let opt = store::select_limit(id, limit);
    if let Some(tuple) = opt {
        let _slice1 = tuple.0;
        let _size1 = tuple.1;
        let _slice2 = tuple.2;
        let _size2 = tuple.3;
        let _size = tuple.4;
        for i in 0.._size1 {
            let _ = _fetch_u64(&_slice1[i * 8..(i + 1) * 8]);
        }
        for i in 0.._size2 {
            let _ = _fetch_u64(&_slice2[i * 8..(i + 1) * 8]);
        }
    }
}
