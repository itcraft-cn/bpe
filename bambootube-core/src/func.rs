use crate::{
    aux::fetch_ptr,
    data::{ColumnType, Record},
    element::Element,
    ffi::FfiFunc,
};
use std::{
    alloc::{alloc, Layout},
    mem::{align_of, size_of_val},
    ptr,
};
use strum_macros::EnumString;

#[derive(Debug, Clone)]
pub(crate) struct Executors {
    raw_ptr: *const Executor,
    size: usize,
}
impl Executors {
    pub(crate) fn new(executors: &[Executor]) -> Self {
        let align_of_executor = align_of::<Executor>();
        let len = executors.len();
        let layout = Layout::from_size_align(size_of_val(executors), align_of_executor).unwrap();
        let raw_ptr = unsafe { alloc(layout) };
        let executor_ptr = raw_ptr.cast::<Executor>();
        for (i, item) in executors.iter().enumerate() {
            unsafe {
                let target_ptr = executor_ptr.add(i);
                ptr::write(target_ptr, item.clone());
            }
        }
        Self {
            raw_ptr: executor_ptr,
            size: len,
        }
    }

    pub(crate) fn executor_size(&self) -> i32 {
        self.size as i32
    }

    pub(crate) fn index_of(&self, idx: i32) -> &Executor {
        unsafe { &*self.raw_ptr.add(idx as usize) }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Executor {
    Fetch(u16, u16),
    Compute(Func, Executors),
}
impl Executor {
    pub(crate) fn fetch(
        &self,
        id: u16,
        u64ptr: u64,
        record: &Record,
        position: usize,
        sub_data_ptr: *const u8,
    ) -> Element {
        match self {
            Executor::Fetch(_, field_id) => {
                fetch_val(sub_data_ptr, id, u64ptr, record, position, *field_id)
            }
            Executor::Compute(f, executors) => {
                compute_func(sub_data_ptr, id, u64ptr, record, position, f, executors)
            }
        }
    }
}

#[inline]
pub(crate) fn fetch_val(
    sub_data_ptr: *const u8,
    _id: u16,
    u64ptr: u64,
    record: &Record,
    position: usize,
    idx: u16,
) -> Element {
    let column = record.column(idx);
    match column.data_type() {
        ColumnType::Long => Element::Long(fetch_ptr(unsafe { sub_data_ptr.add(column.offset()) })),
        ColumnType::Double => {
            Element::Double(fetch_ptr(unsafe { sub_data_ptr.add(column.offset()) }))
        }
        ColumnType::Str(len) => {
            let offset = column.offset();
            Element::Str(u64ptr, position + offset, *len)
        }
    }
}

pub(crate) fn compute_func(
    sub_data_ptr: *const u8,
    id: u16,
    u64ptr: u64,
    record: &Record,
    position: usize,
    f: &Func,
    executors: &Executors,
) -> Element {
    match f {
        Func::Add => add(sub_data_ptr, id, u64ptr, record, position, executors),
        Func::Sub => sub(sub_data_ptr, id, u64ptr, record, position, executors),
        Func::Mul => mul(sub_data_ptr, id, u64ptr, record, position, executors),
        Func::Div => div(sub_data_ptr, id, u64ptr, record, position, executors),
        Func::Mod => mod_(sub_data_ptr, id, u64ptr, record, position, executors),
        _ => Element::Long(0),
    }
}

fn add(
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

fn sub(
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

fn mul(
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

fn div(
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

fn mod_(
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

#[derive(Debug, Clone, EnumString)]
pub(crate) enum Func {
    #[strum(ascii_case_insensitive)]
    Add,
    #[strum(ascii_case_insensitive)]
    Sub,
    #[strum(ascii_case_insensitive)]
    Mul,
    #[strum(ascii_case_insensitive)]
    Div,
    #[strum(ascii_case_insensitive)]
    Mod,
    // aggregate func
    #[strum(ascii_case_insensitive)]
    Key,
    #[strum(ascii_case_insensitive)]
    MinL,
    #[strum(ascii_case_insensitive)]
    MaxL,
    #[strum(ascii_case_insensitive)]
    SumL,
    #[strum(ascii_case_insensitive)]
    Count,
    #[strum(ascii_case_insensitive)]
    MinD,
    #[strum(ascii_case_insensitive)]
    MaxD,
    #[strum(ascii_case_insensitive)]
    SumD,
    #[strum(ascii_case_insensitive)]
    Avg,
}

pub(crate) type NormalFunc = Box<dyn Fn(*const u8, usize) + Send + 'static>;
pub(crate) type LambdaFunc = Box<dyn Fn(*const u8, usize) + 'static>;

pub(crate) enum FnHolder {
    Func(NormalFunc),
    FfiFunc(Box<dyn FfiFunc>),
    Lambda(LambdaFunc),
}
