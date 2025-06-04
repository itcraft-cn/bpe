use crate::{data::Record, element::Element, func::Executors};

pub(crate) fn add(
    sub_data_ptr: *const u8,
    id: u16,
    u64ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if executors.executor_size() != 2 {
        log::warn!(
            "Invalid parameters for add function, should be 2, but was {}",
            executors.executor_size()
        );
        return Element::Long(0);
    }
    let v1 = executors
        .index_of(0)
        .fetch(id, u64ptr, record, position, sub_data_ptr);
    let v2 = executors
        .index_of(1)
        .fetch(id, u64ptr, record, position, sub_data_ptr);
    v1.add(v2)
}

pub(crate) fn sub(
    sub_data_ptr: *const u8,
    id: u16,
    u64ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if executors.executor_size() != 2 {
        log::warn!(
            "Invalid parameters for add function, should be 2, but was {}",
            executors.executor_size()
        );
        return Element::Long(0);
    }
    let v1 = executors
        .index_of(0)
        .fetch(id, u64ptr, record, position, sub_data_ptr);
    let v2 = executors
        .index_of(1)
        .fetch(id, u64ptr, record, position, sub_data_ptr);
    v1.sub(v2)
}

pub(crate) fn mul(
    sub_data_ptr: *const u8,
    id: u16,
    u64ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if executors.executor_size() != 2 {
        log::warn!(
            "Invalid parameters for add function, should be 2, but was {}",
            executors.executor_size()
        );
        return Element::Long(0);
    }
    let v1 = executors
        .index_of(0)
        .fetch(id, u64ptr, record, position, sub_data_ptr);
    let v2 = executors
        .index_of(1)
        .fetch(id, u64ptr, record, position, sub_data_ptr);
    v1.mul(v2)
}

pub(crate) fn div(
    sub_data_ptr: *const u8,
    id: u16,
    u64ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if executors.executor_size() != 2 {
        log::warn!(
            "Invalid parameters for add function, should be 2, but was {}",
            executors.executor_size()
        );
        return Element::Long(0);
    }
    let v1 = executors
        .index_of(0)
        .fetch(id, u64ptr, record, position, sub_data_ptr);
    let v2 = executors
        .index_of(1)
        .fetch(id, u64ptr, record, position, sub_data_ptr);
    v1.div(v2)
}

pub(crate) fn mod_(
    sub_data_ptr: *const u8,
    id: u16,
    u64ptr: u64,
    record: &Record,
    position: usize,
    executors: &Executors,
) -> Element {
    if executors.executor_size() != 2 {
        log::warn!(
            "Invalid parameters for add function, should be 2, but was {}",
            executors.executor_size()
        );
        return Element::Long(0);
    }
    let v1 = executors
        .index_of(0)
        .fetch(id, u64ptr, record, position, sub_data_ptr);
    let v2 = executors
        .index_of(1)
        .fetch(id, u64ptr, record, position, sub_data_ptr);
    v1.mod_(v2)
}
