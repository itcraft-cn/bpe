use crate::{
    aux::fetch_ptr,
    data::Record,
    jit::consts::{T_ERR, T_F64, T_I64},
};
use std::{slice, str::Utf8Error};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct RetVal {
    pub(crate) data_type: u64,
    pub(crate) data: u64,
}
impl RetVal {
    pub(crate) fn new(data_type: u64, data: u64) -> Self {
        Self { data_type, data }
    }
}

pub(crate) unsafe extern "C" fn fetch_column_i64(data_ptr: u64, record_id: u16, column_id: u16) -> RetVal {
    if let Some(column) = Record::get_column(record_id, column_id) {
        let v: i64 = fetch_ptr(unsafe { (data_ptr as *const u8).add(column.offset()) });
        RetVal::new(T_I64, v.cast_unsigned())
    } else {
        log::warn!("cannot found the column({column_id}) in record({record_id})");
        RetVal::new(T_ERR, 0)
    }
}

pub(crate) unsafe extern "C" fn fetch_column_f64(data_ptr: u64, record_id: u16, column_id: u16) -> RetVal {
    if let Some(column) = Record::get_column(record_id, column_id) {
        let v: f64 = fetch_ptr(unsafe { (data_ptr as *const u8).add(column.offset()) });
        RetVal::new(T_F64, v.to_bits())
    } else {
        log::warn!("cannot found the column({column_id}) in record({record_id})");
        RetVal::new(T_ERR, 0)
    }
}

pub(crate) unsafe extern "C" fn int2float(val_type: u64, val: u64) -> f64 {
    if val_type == T_I64 {
        val as f64
    } else if val_type == T_F64 {
        f64::from_bits(val)
    } else {
        log::warn!("invalid val_type: {val_type}/{val}");
        panic!("invalid val_type")
    }
}

pub(crate) unsafe extern "C" fn llvm_log(data: u64, desc: u64, desc_len: u64) {
    let ptr = desc as *const u8;
    if let Ok(msg) = pointer_to_str_safe(ptr, desc_len as usize) {
        log::info!("|jit|[{msg}]=>[{data}]");
    } else {
        log::warn!("cannot convert to string");
    }
}

unsafe fn pointer_to_str_safe(ptr: *const u8, len: usize) -> Result<&'static str, Utf8Error> {
    // 将原始指针转换为字节切片
    let slice = slice::from_raw_parts(ptr, len);
    // 验证并转换为 &str
    str::from_utf8(slice)
}
