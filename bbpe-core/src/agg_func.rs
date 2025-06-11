use crate::aux::{fetch_ptr, fill_ptr};

#[inline]
pub(crate) fn func_max_long(aggregate_data_ptr: *mut u8, offset: usize, v: &i64) {
    let max: i64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, max.max(*v));
}

#[inline]
pub(crate) fn func_min_long(aggregate_data_ptr: *mut u8, offset: usize, v: &i64) {
    let min: i64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, min.min(*v));
}

#[inline]
pub(crate) fn func_sum_long(aggregate_data_ptr: *mut u8, offset: usize, v: &i64) {
    let sum: i64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, sum + *v);
}

#[inline]
pub(crate) fn func_count_long(aggregate_data_ptr: *mut u8, offset: usize) {
    let count: i64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, count + 1);
}

#[inline]
pub(crate) fn func_maxd_long(aggregate_data_ptr: *mut u8, offset: usize, v: &i64) {
    let max: f64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(
        unsafe { aggregate_data_ptr.add(offset) },
        max.max(*v as f64),
    );
}

#[inline]
pub(crate) fn func_mind_long(aggregate_data_ptr: *mut u8, offset: usize, v: &i64) {
    let min: f64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(
        unsafe { aggregate_data_ptr.add(offset) },
        min.min(*v as f64),
    );
}

#[inline]
pub(crate) fn func_sumd_long(aggregate_data_ptr: *mut u8, offset: usize, v: &i64) {
    let sum: f64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, sum + *v as f64);
}

#[inline]
pub(crate) fn func_avg_long(aggregate_data_ptr: *mut u8, offset: usize, v: &i64, data_idx: usize) {
    let avg: f64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(
        unsafe { aggregate_data_ptr.add(offset) },
        (avg * (data_idx as f64) + (*v as f64)) / ((data_idx + 1) as f64),
    );
}

#[inline]
pub(crate) fn func_maxd_double(aggregate_data_ptr: *mut u8, offset: usize, v: &f64) {
    let max: f64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, max.max(*v));
}

#[inline]
pub(crate) fn func_mind_double(aggregate_data_ptr: *mut u8, offset: usize, v: &f64) {
    let min: f64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, min.min(*v));
}

#[inline]
pub(crate) fn func_sumd_double(aggregate_data_ptr: *mut u8, offset: usize, v: &f64) {
    let sum: f64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, sum + *v);
}

#[inline]
pub(crate) fn func_avg_double(
    aggregate_data_ptr: *mut u8,
    offset: usize,
    v: &f64,
    data_idx: usize,
) {
    let avg: f64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(
        unsafe { aggregate_data_ptr.add(offset) },
        (avg * (data_idx as f64) + *v) / ((data_idx + 1) as f64),
    );
}

#[inline]
pub(crate) fn func_first_long(aggregate_data_ptr: *mut u8, offset: usize, v: &i64) {
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, *v);
}

#[inline]
pub(crate) fn func_first_double(aggregate_data_ptr: *mut u8, offset: usize, v: &f64) {
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, *v);
}

#[inline]
pub(crate) fn func_last_long(aggregate_data_ptr: *mut u8, offset: usize, v: &i64) {
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, *v);
}

#[inline]
pub(crate) fn func_last_double(aggregate_data_ptr: *mut u8, offset: usize, v: &f64) {
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, *v);
}
