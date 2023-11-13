use crate::{
    aux::{fetch_f64, fetch_u64},
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
    pub(crate) fn fetch(&self, id: u16, columns: &Vec<Column>, slice: &'static [u8]) -> Element {
        match self {
            Executor::Fetch(_, field_id) => fetch_val(slice, id, columns, *field_id),
            Executor::Compute(f, executors) => compute_func(slice, id, columns, f, executors),
        }
    }
}

#[inline]
pub(crate) fn fetch_val(
    slice: &'static [u8],
    _id: u16,
    columns: &Vec<Column>,
    idx: u16,
) -> Element {
    let column = columns.as_slice()[(idx - 1) as usize];
    match column.data_type() {
        ColumnType::Long => {
            let offset = column.offset();
            Element::Long(fetch_u64(&slice[offset..offset + 8]))
        }
        ColumnType::Double => {
            let offset = column.offset();
            Element::Double(fetch_f64(&slice[offset..offset + 8]))
        }
        ColumnType::Str(len) => {
            let offset = column.offset();
            Element::_Str(&slice[offset..offset + len], *len)
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
    slice: &'static [u8],
    id: u16,
    columns: &Vec<Column>,
    f: &Func,
    executors: &[Executor],
) -> Element {
    match f {
        Func::Add => add(slice, id, columns, executors),
        Func::Sub => sub(slice, id, columns, executors),
        Func::Mul => mul(slice, id, columns, executors),
        Func::Div => div(slice, id, columns, executors),
        Func::Mod => mod_(slice, id, columns, executors),
    }
}

fn add(slice: &'static [u8], id: u16, columns: &Vec<Column>, executors: &[Executor]) -> Element {
    if executors.len() != 2 {
        log::warn!(
            "Invalid parameters for add function, should be 2, but was {}",
            executors.len()
        );
        return Element::Long(0);
    }
    let v1 = executors[0].fetch(id, columns, slice);
    let v2 = executors[1].fetch(id, columns, slice);
    v1.add(v2)
}

fn sub(slice: &'static [u8], id: u16, columns: &Vec<Column>, executors: &[Executor]) -> Element {
    if executors.len() != 2 {
        log::warn!(
            "Invalid parameters for add function, should be 2, but was {}",
            executors.len()
        );
        return Element::Long(0);
    }
    let v1 = executors[0].fetch(id, columns, slice);
    let v2 = executors[1].fetch(id, columns, slice);
    v1.sub(v2)
}

fn mul(slice: &'static [u8], id: u16, columns: &Vec<Column>, executors: &[Executor]) -> Element {
    if executors.len() != 2 {
        log::warn!(
            "Invalid parameters for add function, should be 2, but was {}",
            executors.len()
        );
        return Element::Long(0);
    }
    let v1 = executors[0].fetch(id, columns, slice);
    let v2 = executors[1].fetch(id, columns, slice);
    v1.mul(v2)
}

fn div(slice: &'static [u8], id: u16, columns: &Vec<Column>, executors: &[Executor]) -> Element {
    if executors.len() != 2 {
        log::warn!(
            "Invalid parameters for add function, should be 2, but was {}",
            executors.len()
        );
        return Element::Long(0);
    }
    let v1 = executors[0].fetch(id, columns, slice);
    let v2 = executors[1].fetch(id, columns, slice);
    v1.div(v2)
}

fn mod_(slice: &'static [u8], id: u16, columns: &Vec<Column>, executors: &[Executor]) -> Element {
    if executors.len() != 2 {
        log::warn!(
            "Invalid parameters for add function, should be 2, but was {}",
            executors.len()
        );
        return Element::Long(0);
    }
    let v1 = executors[0].fetch(id, columns, slice);
    let v2 = executors[1].fetch(id, columns, slice);
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
}

pub(crate) enum FnHolder {
    NotExist,
    Func(Box<dyn Fn(Vec<[u8; 512]>) + Send + 'static>),
    FfiFunc(Box<dyn FfiFunc>),
}
