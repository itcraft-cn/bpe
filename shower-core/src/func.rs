use crate::{
    aux::{fetch_f64, fetch_i64},
    data::{Column, ColumnType},
    element::Element,
    ffi::FfiFunc,
};
use strum_macros::EnumString;

#[derive(Debug, Clone)]
pub(crate) enum Executor {
    Fetch(u16, u16),
    Compute(Func, Vec<Executor>),
}
impl Executor {
    pub(crate) fn fetch(
        &self,
        id: u16,
        u64ptr: u64,
        columns: &Vec<Column>,
        position: usize,
        slice: &[u8],
    ) -> Element {
        match self {
            Executor::Fetch(_, field_id) => {
                fetch_val(slice, id, u64ptr, columns, position, *field_id)
            }
            Executor::Compute(f, executors) => {
                compute_func(slice, id, u64ptr, columns, position, f, executors)
            }
        }
    }
}

#[inline]
pub(crate) fn fetch_val(
    slice: &[u8],
    _id: u16,
    u64ptr: u64,
    columns: &[Column],
    position: usize,
    idx: u16,
) -> Element {
    // TODO: remove unwrap
    let column = columns.get((idx - 1) as usize).unwrap();
    match column.data_type() {
        ColumnType::Long => {
            let offset = column.offset();
            Element::Long(fetch_i64(&slice[offset..offset + 8]))
        }
        ColumnType::Double => {
            let offset = column.offset();
            Element::Double(fetch_f64(&slice[offset..offset + 8]))
        }
        ColumnType::Str(len) => {
            let offset = column.offset();
            Element::Str(u64ptr, position + offset, *len)
        }
    }
}

#[inline]
pub(crate) fn eq(expacted: Element, val: Element) -> bool {
    val.eq(expacted)
}
#[inline]
pub(crate) fn gt_eq(expacted: Element, val: Element) -> bool {
    val.gt_eq(expacted)
}
#[inline]
pub(crate) fn gt(expacted: Element, val: Element) -> bool {
    val.gt(expacted)
}
#[inline]
pub(crate) fn lt_eq(expacted: Element, val: Element) -> bool {
    val.lt_eq(expacted)
}
#[inline]
pub(crate) fn lt(expacted: Element, val: Element) -> bool {
    val.lt(expacted)
}
#[inline]
pub(crate) fn neq(expacted: Element, val: Element) -> bool {
    val.neq(expacted)
}

pub(crate) fn compute_func(
    slice: &[u8],
    id: u16,
    u64ptr: u64,
    columns: &Vec<Column>,
    position: usize,
    f: &Func,
    executors: &[Executor],
) -> Element {
    match f {
        Func::Add => add(slice, id, u64ptr, columns, position, executors),
        Func::Sub => sub(slice, id, u64ptr, columns, position, executors),
        Func::Mul => mul(slice, id, u64ptr, columns, position, executors),
        Func::Div => div(slice, id, u64ptr, columns, position, executors),
        Func::Mod => mod_(slice, id, u64ptr, columns, position, executors),
        _ => Element::Long(0),
    }
}

fn add(
    slice: &[u8],
    id: u16,
    u64ptr: u64,
    columns: &Vec<Column>,
    position: usize,
    executors: &[Executor],
) -> Element {
    if executors.len() != 2 {
        log::warn!(
            "Invalid parameters for add function, should be 2, but was {}",
            executors.len()
        );
        return Element::Long(0);
    }
    let v1 = executors[0].fetch(id, u64ptr, columns, position, slice);
    let v2 = executors[1].fetch(id, u64ptr, columns, position, slice);
    v1.add(v2)
}

fn sub(
    slice: &[u8],
    id: u16,
    u64ptr: u64,
    columns: &Vec<Column>,
    position: usize,
    executors: &[Executor],
) -> Element {
    if executors.len() != 2 {
        log::warn!(
            "Invalid parameters for add function, should be 2, but was {}",
            executors.len()
        );
        return Element::Long(0);
    }
    let v1 = executors[0].fetch(id, u64ptr, columns, position, slice);
    let v2 = executors[1].fetch(id, u64ptr, columns, position, slice);
    v1.sub(v2)
}

fn mul(
    slice: &[u8],
    id: u16,
    u64ptr: u64,
    columns: &Vec<Column>,
    position: usize,
    executors: &[Executor],
) -> Element {
    if executors.len() != 2 {
        log::warn!(
            "Invalid parameters for add function, should be 2, but was {}",
            executors.len()
        );
        return Element::Long(0);
    }
    let v1 = executors[0].fetch(id, u64ptr, columns, position, slice);
    let v2 = executors[1].fetch(id, u64ptr, columns, position, slice);
    v1.mul(v2)
}

fn div(
    slice: &[u8],
    id: u16,
    u64ptr: u64,
    columns: &Vec<Column>,
    position: usize,
    executors: &[Executor],
) -> Element {
    if executors.len() != 2 {
        log::warn!(
            "Invalid parameters for add function, should be 2, but was {}",
            executors.len()
        );
        return Element::Long(0);
    }
    let v1 = executors[0].fetch(id, u64ptr, columns, position, slice);
    let v2 = executors[1].fetch(id, u64ptr, columns, position, slice);
    v1.div(v2)
}

fn mod_(
    slice: &[u8],
    id: u16,
    u64ptr: u64,
    columns: &Vec<Column>,
    position: usize,
    executors: &[Executor],
) -> Element {
    if executors.len() != 2 {
        log::warn!(
            "Invalid parameters for add function, should be 2, but was {}",
            executors.len()
        );
        return Element::Long(0);
    }
    let v1 = executors[0].fetch(id, u64ptr, columns, position, slice);
    let v2 = executors[1].fetch(id, u64ptr, columns, position, slice);
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

pub(crate) enum FnHolder {
    Func(Box<dyn Fn(&Vec<[u8; 512]>) + Send + 'static>),
    FfiFunc(Box<dyn FfiFunc>),
    Lambda(Box<dyn Fn(&Vec<[u8; 512]>) + 'static>),
}
