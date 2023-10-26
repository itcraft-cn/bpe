use crate::{aux::fetch_u64, ffi::FfiFunc};
use strum_macros::EnumString;

#[derive(Debug, Clone)]
pub(crate) enum Executor {
    Fetch(u16, u16),
    Compute(Func, Vec<Executor>),
}
impl Executor {
    pub(crate) fn fetch(&self, slice: &[u8]) -> u64 {
        match self {
            Executor::Fetch(_, field_id) => fetch_val(slice, *field_id),
            Executor::Compute(f, executors) => compute_func(slice, f, executors),
        }
    }
}

#[inline]
pub(crate) fn fetch_val(slice: &[u8], idx: u16) -> u64 {
    let real_idx = idx - 1;
    fetch_u64(&slice[real_idx as usize * 8..idx as usize * 8])
}

#[inline]
pub(crate) fn eq(expacted: u64, val: u64) -> bool {
    val == expacted
}
#[inline]
pub(crate) fn gt_eq(expacted: u64, val: u64) -> bool {
    val >= expacted
}
#[inline]
pub(crate) fn gt(expacted: u64, val: u64) -> bool {
    val > expacted
}
#[inline]
pub(crate) fn lt_eq(expacted: u64, val: u64) -> bool {
    val <= expacted
}
#[inline]
pub(crate) fn lt(expacted: u64, val: u64) -> bool {
    val < expacted
}
#[inline]
pub(crate) fn neq(expacted: u64, val: u64) -> bool {
    val != expacted
}

pub(crate) fn compute_func(slice: &[u8], f: &Func, executors: &[Executor]) -> u64 {
    match f {
        Func::Add => add(slice, executors),
        Func::Sub => sub(slice, executors),
        Func::Mul => mul(slice, executors),
        Func::Div => div(slice, executors),
        Func::Mod => mod_(slice, executors),
    }
}

fn add(slice: &[u8], executors: &[Executor]) -> u64 {
    if executors.len() != 2 {
        log::warn!(
            "Invalid parameters for add function, should be 2, but was {}",
            executors.len()
        );
        return 0;
    }
    let v1 = executors[0].fetch(slice);
    let v2 = executors[1].fetch(slice);
    v1 + v2
}

fn sub(slice: &[u8], executors: &[Executor]) -> u64 {
    if executors.len() != 2 {
        log::warn!(
            "Invalid parameters for add function, should be 2, but was {}",
            executors.len()
        );
        return 0;
    }
    let v1 = executors[0].fetch(slice);
    let v2 = executors[1].fetch(slice);
    v1 - v2
}

fn mul(slice: &[u8], executors: &[Executor]) -> u64 {
    if executors.len() != 2 {
        log::warn!(
            "Invalid parameters for add function, should be 2, but was {}",
            executors.len()
        );
        return 0;
    }
    let v1 = executors[0].fetch(slice);
    let v2 = executors[1].fetch(slice);
    v1 * v2
}

fn div(slice: &[u8], executors: &[Executor]) -> u64 {
    if executors.len() != 2 {
        log::warn!(
            "Invalid parameters for add function, should be 2, but was {}",
            executors.len()
        );
        return 0;
    }
    let v1 = executors[0].fetch(slice);
    let v2 = executors[1].fetch(slice);
    v1 / v2
}

fn mod_(slice: &[u8], executors: &[Executor]) -> u64 {
    if executors.len() != 2 {
        log::warn!(
            "Invalid parameters for add function, should be 2, but was {}",
            executors.len()
        );
        return 0;
    }
    let v1 = executors[0].fetch(slice);
    let v2 = executors[1].fetch(slice);
    v1 % v2
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
    Func(Box<dyn Fn(Vec<[u64; 64]>) + Send + 'static>),
    FfiFunc(Box<dyn FfiFunc>),
}
