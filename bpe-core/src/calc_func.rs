use crate::{data::Record, element::Element, exec::Executors};

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
            "Invalid parameters for add function, should be 2, but was {}",
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
