use crate::{
    aux::fetch_ptr,
    calc_func,
    data::{ColumnType, Record},
    element::Element,
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
    ConstLong(i64),
    ConstDouble(f64),
    Fetch(u16, u16),
    Compute(Func, Executors),
}
impl Executor {
    pub(crate) fn fetch(
        &self,
        id: u16,
        v_ptr: u64,
        record: &Record,
        position: usize,
        sub_data_ptr: *const u8,
    ) -> Element {
        match self {
            Executor::ConstLong(v) => Element::Long(*v),
            Executor::ConstDouble(v) => Element::Double(*v),
            Executor::Fetch(_, field_id) => {
                fetch_val(sub_data_ptr, id, v_ptr, record, position, *field_id)
            }
            Executor::Compute(f, executors) => {
                compute_func(sub_data_ptr, id, v_ptr, record, position, f, executors)
            }
        }
    }
}

#[inline]
pub(crate) fn fetch_val(
    sub_data_ptr: *const u8,
    _id: u16,
    _v_ptr: u64,
    record: &Record,
    _position: usize,
    idx: u16,
) -> Element {
    let column = record.column(idx);
    match column.data_type() {
        ColumnType::Long => Element::Long(fetch_ptr(unsafe { sub_data_ptr.add(column.offset()) })),
        ColumnType::Double => {
            Element::Double(fetch_ptr(unsafe { sub_data_ptr.add(column.offset()) }))
        }
    }
}

pub(crate) fn compute_func(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    f: &Func,
    executors: &Executors,
) -> Element {
    match f {
        Func::Add => calc_func::add(sub_data_ptr, id, v_ptr, record, position, executors),
        Func::Sub => calc_func::sub(sub_data_ptr, id, v_ptr, record, position, executors),
        Func::Mul => calc_func::mul(sub_data_ptr, id, v_ptr, record, position, executors),
        Func::Div => calc_func::div(sub_data_ptr, id, v_ptr, record, position, executors),
        Func::Mod => calc_func::mod_(sub_data_ptr, id, v_ptr, record, position, executors),
        _ => Element::Long(0),
    }
}

#[derive(Debug, Clone, EnumString)]
pub(crate) enum Func {
    // calc func
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
    #[strum(ascii_case_insensitive)]
    FirstL,
    #[strum(ascii_case_insensitive)]
    FirstD,
    #[strum(ascii_case_insensitive)]
    LastL,
    #[strum(ascii_case_insensitive)]
    LastD,
}
