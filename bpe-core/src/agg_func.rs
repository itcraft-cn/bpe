use crate::aux::{fetch_ptr, fill_ptr};

//#[inline]
pub(crate) fn f_max_l(aggregate_data_ptr: *mut u8, offset: usize, v: &i64) {
    let max: i64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, max.max(*v));
}

//#[inline]
pub(crate) fn f_min_l(aggregate_data_ptr: *mut u8, offset: usize, v: &i64) {
    let min: i64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, min.min(*v));
}

//#[inline]
pub(crate) fn f_sum_l(aggregate_data_ptr: *mut u8, offset: usize, v: &i64) {
    let sum: i64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, sum + *v);
}

//#[inline]
pub(crate) fn f_count_l(aggregate_data_ptr: *mut u8, offset: usize) {
    let count: i64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, count + 1);
}

//#[inline]
pub(crate) fn f_max_d_l(aggregate_data_ptr: *mut u8, offset: usize, v: &i64) {
    let max: f64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(
        unsafe { aggregate_data_ptr.add(offset) },
        max.max(*v as f64),
    );
}

//#[inline]
pub(crate) fn f_min_d_l(aggregate_data_ptr: *mut u8, offset: usize, v: &i64) {
    let min: f64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(
        unsafe { aggregate_data_ptr.add(offset) },
        min.min(*v as f64),
    );
}

//#[inline]
pub(crate) fn f_sum_d_l(aggregate_data_ptr: *mut u8, offset: usize, v: &i64) {
    let sum: f64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, sum + *v as f64);
}

//#[inline]
pub(crate) fn f_avg_l(aggregate_data_ptr: *mut u8, offset: usize, v: &i64, data_idx: usize) {
    let avg: f64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(
        unsafe { aggregate_data_ptr.add(offset) },
        (avg * (data_idx as f64) + (*v as f64)) / ((data_idx + 1) as f64),
    );
}

//#[inline]
pub(crate) fn f_max_d_d(aggregate_data_ptr: *mut u8, offset: usize, v: &f64) {
    let max: f64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, max.max(*v));
}

//#[inline]
pub(crate) fn f_min_d_d(aggregate_data_ptr: *mut u8, offset: usize, v: &f64) {
    let min: f64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, min.min(*v));
}

//#[inline]
pub(crate) fn f_sum_d_d(aggregate_data_ptr: *mut u8, offset: usize, v: &f64) {
    let sum: f64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, sum + *v);
}

//#[inline]
pub(crate) fn f_avg_d(
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

//#[inline]
pub(crate) fn f_first_l(aggregate_data_ptr: *mut u8, offset: usize, v: &i64) {
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, *v);
}

//#[inline]
pub(crate) fn f_first_d(aggregate_data_ptr: *mut u8, offset: usize, v: &f64) {
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, *v);
}

//#[inline]
pub(crate) fn f_last_l(aggregate_data_ptr: *mut u8, offset: usize, v: &i64) {
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, *v);
}

//#[inline]
pub(crate) fn f_last_d(aggregate_data_ptr: *mut u8, offset: usize, v: &f64) {
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, *v);
}

// ============================================================================
// Variance / stddev via Welford's online algorithm.
// State layout (24 bytes at state_ptr): [count: f64][mean: f64][m2: f64]
// ============================================================================

//#[inline]
pub(crate) fn f_stddev_step(state_ptr: *mut u8, v: f64) {
    let count: f64 = fetch_ptr(state_ptr);
    let mean: f64 = fetch_ptr(unsafe { state_ptr.add(8) });
    let m2: f64 = fetch_ptr(unsafe { state_ptr.add(16) });
    let n = count + 1.0;
    let delta = v - mean;
    let new_mean = mean + delta / n;
    let new_m2 = m2 + delta * (v - new_mean);
    fill_ptr(state_ptr, n);
    fill_ptr(unsafe { state_ptr.add(8) }, new_mean);
    fill_ptr(unsafe { state_ptr.add(16) }, new_m2);
}

/// Writes the final variance into the output slot. `sample=true` uses n-1.
//#[inline]
pub(crate) fn f_var_finalize(out: *mut u8, state_ptr: *const u8, sample: bool) {
    let count: f64 = fetch_ptr(state_ptr);
    let m2: f64 = fetch_ptr(unsafe { state_ptr.add(16) });
    if count <= 1.0 {
        // undefined for n < 2 (population var of a single value is 0, sample is undefined)
        fill_ptr(out, 0.0_f64);
        return;
    }
    let var = if sample { m2 / (count - 1.0) } else { m2 / count };
    fill_ptr(out, var);
}

/// Writes the final stddev into the output slot.
//#[inline]
pub(crate) fn f_stddev_finalize(out: *mut u8, state_ptr: *const u8, sample: bool) {
    let count: f64 = fetch_ptr(state_ptr);
    let m2: f64 = fetch_ptr(unsafe { state_ptr.add(16) });
    if count <= 1.0 {
        fill_ptr(out, 0.0_f64);
        return;
    }
    let var = if sample { m2 / (count - 1.0) } else { m2 / count };
    fill_ptr(out, var.sqrt());
}
