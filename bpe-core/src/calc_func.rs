use crate::{data::Record, dimension, element::Element, exec::Executors};

// ============================================================================
// Pure element-level operations. Shared by the interpreter (exec.rs) and the
// JIT-compiled filter functions (jit/aux.rs), so both paths are always in sync.
// ============================================================================

pub(crate) fn abs_elem(e: Element) -> Element {
    match e {
        Element::Long(v) => Element::Long(v.abs()),
        Element::Double(v) => Element::Double(v.abs()),
    }
}

pub(crate) fn ceil_elem(e: Element) -> Element {
    match e {
        Element::Long(v) => Element::Long(v),
        Element::Double(v) => Element::Double(v.ceil()),
    }
}

pub(crate) fn floor_elem(e: Element) -> Element {
    match e {
        Element::Long(v) => Element::Long(v),
        Element::Double(v) => Element::Double(v.floor()),
    }
}

pub(crate) fn round_elem(e: Element) -> Element {
    match e {
        Element::Long(v) => Element::Long(v),
        Element::Double(v) => Element::Double(v.round()),
    }
}

pub(crate) fn trunc_elem(e: Element) -> Element {
    match e {
        Element::Long(v) => Element::Long(v),
        Element::Double(v) => Element::Double(v.trunc()),
    }
}

pub(crate) fn sign_elem(e: Element) -> Element {
    match e {
        Element::Long(v) => Element::Long(v.signum()),
        Element::Double(v) => Element::Double(v.signum()),
    }
}

pub(crate) fn sqrt_elem(e: Element) -> Element {
    Element::Double(to_f64(e).sqrt())
}

pub(crate) fn exp_elem(e: Element) -> Element {
    Element::Double(to_f64(e).exp())
}

pub(crate) fn ln_elem(e: Element) -> Element {
    Element::Double(to_f64(e).ln())
}

pub(crate) fn log10_elem(e: Element) -> Element {
    Element::Double(to_f64(e).log10())
}

pub(crate) fn to_long_elem(e: Element) -> Element {
    match e {
        Element::Long(v) => Element::Long(v),
        Element::Double(v) => Element::Long(v as i64),
    }
}

pub(crate) fn to_double_elem(e: Element) -> Element {
    Element::Double(to_f64(e))
}

pub(crate) fn pow_elem(a: Element, b: Element) -> Element {
    Element::Double(to_f64(a).powf(to_f64(b)))
}

pub(crate) fn greatest_elem(a: Element, b: Element) -> Element {
    match (a, b) {
        (Element::Long(v1), Element::Long(v2)) => Element::Long(v1.max(v2)),
        (Element::Long(v1), Element::Double(v2)) => Element::Double((v1 as f64).max(v2)),
        (Element::Double(v1), Element::Long(v2)) => Element::Double(v1.max(v2 as f64)),
        (Element::Double(v1), Element::Double(v2)) => Element::Double(v1.max(v2)),
    }
}

pub(crate) fn least_elem(a: Element, b: Element) -> Element {
    match (a, b) {
        (Element::Long(v1), Element::Long(v2)) => Element::Long(v1.min(v2)),
        (Element::Long(v1), Element::Double(v2)) => Element::Double((v1 as f64).min(v2)),
        (Element::Double(v1), Element::Long(v2)) => Element::Double(v1.min(v2 as f64)),
        (Element::Double(v1), Element::Double(v2)) => Element::Double(v1.min(v2)),
    }
}

//#[inline]
fn to_f64(e: Element) -> f64 {
    match e {
        Element::Long(v) => v as f64,
        Element::Double(v) => v,
    }
}

// ============================================================================
// Interpreter entry points. Each fetches the operands from the executor tree and
// delegates to the pure element-level operations above.
// ============================================================================

pub(crate) fn add(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if let Some((v1, v2)) = fetch_2_arg(sub_data_ptr, id, v_ptr, record, position, executors) {
        v1.add(v2)
    } else {
        Element::Long(0)
    }
}

pub(crate) fn sub(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if let Some((v1, v2)) = fetch_2_arg(sub_data_ptr, id, v_ptr, record, position, executors) {
        v1.sub(v2)
    } else {
        Element::Long(0)
    }
}

pub(crate) fn mul(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if let Some((v1, v2)) = fetch_2_arg(sub_data_ptr, id, v_ptr, record, position, executors) {
        v1.mul(v2)
    } else {
        Element::Long(0)
    }
}

pub(crate) fn div(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if let Some((v1, v2)) = fetch_2_arg(sub_data_ptr, id, v_ptr, record, position, executors) {
        v1.div(v2)
    } else {
        Element::Long(0)
    }
}

pub(crate) fn mod_(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if let Some((v1, v2)) = fetch_2_arg(sub_data_ptr, id, v_ptr, record, position, executors) {
        v1.mod_(v2)
    } else {
        Element::Long(0)
    }
}

pub(crate) fn abs(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if let Some(v) = fetch_1_arg(sub_data_ptr, id, v_ptr, record, position, executors) {
        abs_elem(v)
    } else {
        Element::Long(0)
    }
}

pub(crate) fn ceil(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if let Some(v) = fetch_1_arg(sub_data_ptr, id, v_ptr, record, position, executors) {
        ceil_elem(v)
    } else {
        Element::Long(0)
    }
}

pub(crate) fn floor(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if let Some(v) = fetch_1_arg(sub_data_ptr, id, v_ptr, record, position, executors) {
        floor_elem(v)
    } else {
        Element::Long(0)
    }
}

pub(crate) fn round(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if let Some(v) = fetch_1_arg(sub_data_ptr, id, v_ptr, record, position, executors) {
        round_elem(v)
    } else {
        Element::Long(0)
    }
}

pub(crate) fn trunc(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if let Some(v) = fetch_1_arg(sub_data_ptr, id, v_ptr, record, position, executors) {
        trunc_elem(v)
    } else {
        Element::Long(0)
    }
}

pub(crate) fn sign(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if let Some(v) = fetch_1_arg(sub_data_ptr, id, v_ptr, record, position, executors) {
        sign_elem(v)
    } else {
        Element::Long(0)
    }
}

pub(crate) fn sqrt(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if let Some(v) = fetch_1_arg(sub_data_ptr, id, v_ptr, record, position, executors) {
        sqrt_elem(v)
    } else {
        Element::Long(0)
    }
}

pub(crate) fn exp(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if let Some(v) = fetch_1_arg(sub_data_ptr, id, v_ptr, record, position, executors) {
        exp_elem(v)
    } else {
        Element::Long(0)
    }
}

pub(crate) fn ln(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if let Some(v) = fetch_1_arg(sub_data_ptr, id, v_ptr, record, position, executors) {
        ln_elem(v)
    } else {
        Element::Long(0)
    }
}

pub(crate) fn log10(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if let Some(v) = fetch_1_arg(sub_data_ptr, id, v_ptr, record, position, executors) {
        log10_elem(v)
    } else {
        Element::Long(0)
    }
}

pub(crate) fn to_long(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if let Some(v) = fetch_1_arg(sub_data_ptr, id, v_ptr, record, position, executors) {
        to_long_elem(v)
    } else {
        Element::Long(0)
    }
}

pub(crate) fn to_double(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if let Some(v) = fetch_1_arg(sub_data_ptr, id, v_ptr, record, position, executors) {
        to_double_elem(v)
    } else {
        Element::Long(0)
    }
}

pub(crate) fn pow(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if let Some((v1, v2)) = fetch_2_arg(sub_data_ptr, id, v_ptr, record, position, executors) {
        pow_elem(v1, v2)
    } else {
        Element::Long(0)
    }
}

pub(crate) fn greatest(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if let Some((v1, v2)) = fetch_2_arg(sub_data_ptr, id, v_ptr, record, position, executors) {
        greatest_elem(v1, v2)
    } else {
        Element::Long(0)
    }
}

pub(crate) fn least(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if let Some((v1, v2)) = fetch_2_arg(sub_data_ptr, id, v_ptr, record, position, executors) {
        least_elem(v1, v2)
    } else {
        Element::Long(0)
    }
}

/// `_dim_has(dim_id, key) -> Long(1/0)`: key exists in the dimension table.
pub(crate) fn dim_has(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if let Some((dim_id, key)) = fetch_2_arg(sub_data_ptr, id, v_ptr, record, position, executors) {
        let did = elem_to_dim_id(dim_id);
        let k = elem_to_i64(key);
        Element::Long(if dimension::dim_contains(did, k) { 1 } else { 0 })
    } else {
        Element::Long(0)
    }
}

/// `_dim_get(dim_id, key) -> Long(value)`: value for the key, 0 if absent.
pub(crate) fn dim_get(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if let Some((dim_id, key)) = fetch_2_arg(sub_data_ptr, id, v_ptr, record, position, executors) {
        let did = elem_to_dim_id(dim_id);
        let k = elem_to_i64(key);
        Element::Long(dimension::dim_get(did, k))
    } else {
        Element::Long(0)
    }
}

fn elem_to_dim_id(e: Element) -> u16 {
    match e {
        Element::Long(v) => v as u16,
        Element::Double(v) => v as u16,
    }
}

fn elem_to_i64(e: Element) -> i64 {
    match e {
        Element::Long(v) => v,
        Element::Double(v) => v as i64,
    }
}

fn fetch_1_arg(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Option<Element> {
    if executors.executor_size() != 1 {
        log::warn!(
            "Invalid parameters for function, should be 1, but was {}",
            executors.executor_size()
        );
        None
    } else {
        Some(
            executors
                .index_of(0)
                .fetch(id, v_ptr, record, position, sub_data_ptr),
        )
    }
}

fn fetch_2_arg(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Option<(Element, Element)> {
    if executors.executor_size() != 2 {
        log::warn!(
            "Invalid parameters for function, should be 2, but was {}",
            executors.executor_size()
        );
        None
    } else {
        Some((
            executors
                .index_of(0)
                .fetch(id, v_ptr, record, position, sub_data_ptr),
            executors
                .index_of(1)
                .fetch(id, v_ptr, record, position, sub_data_ptr),
        ))
    }
}
