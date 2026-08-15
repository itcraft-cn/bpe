use crate::{
    aux::fetch_ptr,
    calc_func,
    data::Record,
    element::Element,
    jit::consts::{T_ERR, T_F64, T_I64},
    Column,
};
use std::{slice, str::Utf8Error};

/// Represents a return value from a JIT function with a type identifier and the actual data.
/// Used for returning values from column fetch functions.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct RetVal {
    pub(crate) data_type: u64, // Type identifier (T_I64, T_F64, T_ERR, etc.)
    pub(crate) data: u64,      // The actual data value
}
impl RetVal {
    /// Creates a new RetVal with the specified type and data.
    pub(crate) fn new(data_type: u64, data: u64) -> Self {
        Self { data_type, data }
    }
}

type FetchColumnFunc = dyn Fn(u64, &'static Column) -> u64;

/// Fetches an integer value from a column in the specified record.
/// This function is called from generated LLVM code to access column data.
pub(crate) unsafe extern "C" fn fetch_column_i64(
    data_ptr: u64,
    record_id: u16,
    column_id: u16,
) -> RetVal {
    fetch_column(data_ptr, record_id, column_id, &fetch_column_i64_val, T_I64)
}

/// Fetches a float value from a column in the specified record.
/// This function is called from generated LLVM code to access column data.
pub(crate) unsafe extern "C" fn fetch_column_f64(
    data_ptr: u64,
    record_id: u16,
    column_id: u16,
) -> RetVal {
    fetch_column(data_ptr, record_id, column_id, &fetch_column_f64_val, T_F64)
}

fn fetch_column_i64_val(data_ptr: u64, column: &'static Column) -> u64 {
    // Calculate the memory address of the column data
    let v: i64 = fetch_ptr(unsafe { (data_ptr as *const u8).add(column.offset()) });
    v.cast_unsigned()
}

fn fetch_column_f64_val(data_ptr: u64, column: &'static Column) -> u64 {
    // Calculate the memory address of the column data
    let v: f64 = fetch_ptr(unsafe { (data_ptr as *const u8).add(column.offset()) });
    v.to_bits()
}

fn fetch_column(
    data_ptr: u64,
    record_id: u16,
    column_id: u16,
    func: &FetchColumnFunc,
    val_type: u64,
) -> RetVal {
    if let Some(column) = Record::get_column(record_id, column_id) {
        // Return the value with type identifier for float (using bits representation)
        RetVal::new(val_type, func(data_ptr, column))
    } else {
        log::warn!("cannot found the column({column_id}) in record({record_id})"); // Log warning if column not found
        RetVal::new(T_ERR, 0) // Return error value
    }
}

/// Converts a value to a float based on its type identifier.
/// Used for type conversion in generated LLVM code when comparing mixed types.
pub(crate) unsafe extern "C" fn int2float(val_type: u64, val: u64) -> f64 {
    if val_type == T_I64 {
        // val holds the bit pattern of an i64; reinterpret as signed before converting,
        // otherwise negative integers become huge positive floats
        val as i64 as f64
    } else if val_type == T_F64 {
        f64::from_bits(val) // Reinterpret float bits as float value
    } else {
        log::warn!("invalid val_type: {val_type}/{val}"); // Log warning for invalid type
        panic!("invalid val_type") // Panic for invalid type
    }
}

/// Logs a message from generated LLVM code, primarily used for debugging JIT-compiled functions.
pub(crate) unsafe extern "C" fn llvm_log_int(data: u64, desc: u64, desc_len: u64) {
    log_val!(data, desc, desc_len);
}

/// Logs a message from generated LLVM code, primarily used for debugging JIT-compiled functions.
pub(crate) unsafe extern "C" fn llvm_log_float(data: f64, desc: u64, desc_len: u64) {
    log_val!(data, desc, desc_len);
}

/// Safely converts a raw pointer and length to a string slice, checking for valid UTF-8.
unsafe fn pointer_to_str_safe(ptr: *const u8, len: usize) -> Result<&'static str, Utf8Error> {
    // Convert raw pointer to byte slice
    let slice = slice::from_raw_parts(ptr, len);
    // Validate and convert to string slice
    str::from_utf8(slice)
}

// ============================================================================
// Scalar function support for the JIT filter path.
// Each scalar function is registered in the LLVM module as `func_{name}` with
// signature `(i64 type1, i64 val1[, i64 type2, i64 val2]) -> RetVal{type, value}`.
// The implementation delegates to the pure element-level ops in calc_func, so
// the JIT path and the interpreter path (exec.rs) always agree.
// ============================================================================

/// Converts a {type tag, bit pattern} pair into an Element.
fn element_from_bits(type_tag: u64, bits: u64) -> Element {
    match type_tag {
        T_I64 => Element::Long(bits as i64),
        T_F64 => Element::Double(f64::from_bits(bits)),
        _ => {
            log::warn!("invalid element type tag: {type_tag}");
            Element::Long(0)
        }
    }
}

/// Converts an Element into a {type tag, bit pattern} RetVal.
fn retval_from_element(e: Element) -> RetVal {
    match e {
        Element::Long(v) => RetVal::new(T_I64, v as u64),
        Element::Double(v) => RetVal::new(T_F64, v.to_bits()),
    }
}

macro_rules! jit_unary {
    ($fname:ident, $op:expr) => {
        pub(crate) unsafe extern "C" fn $fname(type1: u64, val1: u64) -> RetVal {
            retval_from_element(($op)(element_from_bits(type1, val1)))
        }
    };
}

macro_rules! jit_binary {
    ($fname:ident, $op:expr) => {
        pub(crate) unsafe extern "C" fn $fname(type1: u64, val1: u64, type2: u64, val2: u64) -> RetVal {
            retval_from_element(($op)(
                element_from_bits(type1, val1),
                element_from_bits(type2, val2),
            ))
        }
    };
}

jit_unary!(jit_abs, calc_func::abs_elem);
jit_unary!(jit_ceil, calc_func::ceil_elem);
jit_unary!(jit_floor, calc_func::floor_elem);
jit_unary!(jit_round, calc_func::round_elem);
jit_unary!(jit_trunc, calc_func::trunc_elem);
jit_unary!(jit_sign, calc_func::sign_elem);
jit_unary!(jit_sqrt, calc_func::sqrt_elem);
jit_unary!(jit_exp, calc_func::exp_elem);
jit_unary!(jit_ln, calc_func::ln_elem);
jit_unary!(jit_log10, calc_func::log10_elem);
jit_unary!(jit_to_long, calc_func::to_long_elem);
jit_unary!(jit_to_double, calc_func::to_double_elem);

jit_binary!(jit_add, |a: Element, b: Element| a.add(b));
jit_binary!(jit_sub, |a: Element, b: Element| a.sub(b));
jit_binary!(jit_mul, |a: Element, b: Element| a.mul(b));
jit_binary!(jit_div, |a: Element, b: Element| a.div(b));
jit_binary!(jit_mod, |a: Element, b: Element| a.mod_(b));
jit_binary!(jit_pow, calc_func::pow_elem);
jit_binary!(jit_greatest, calc_func::greatest_elem);
jit_binary!(jit_least, calc_func::least_elem);
