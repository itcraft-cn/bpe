use crate::{aux::fetch_ptr, data::Record};
use globalvar::{def_global_ptr, get_global_mut};
use inkwell::{
    builder::Builder,
    context::Context,
    execution_engine::{ExecutionEngine, JitFunction, UnsafeFunctionPointer},
    module::Module,
    types::{FunctionType, StructType},
    OptimizationLevel,
};

static mut PTR_LLVM_CTX_STORE: u64 = 0;

pub const T_ERR: u8 = 0;
pub const T_I64: u8 = 1;
pub const T_F64: u8 = 2;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RetVal {
    pub data_type: u8,
    pub data: u64,
}
impl RetVal {
    pub fn new(data_type: u8, data: u64) -> Self {
        Self { data_type, data }
    }
}

#[derive(Debug)]
pub(crate) struct FuncGenerator<'ctx> {
    pub(crate) context: &'ctx Context,
    pub(crate) module: Module<'ctx>,
    pub(crate) builder: Builder<'ctx>,
    pub(crate) execution_engine: ExecutionEngine<'ctx>,
    pub(crate) type_wrapper: Option<StructType<'ctx>>,
}
impl FuncGenerator<'_> {
    pub(crate) fn new() -> Self {
        let context = get_ctx();
        let module = context.create_module("llvm_jit");
        let rs_engine = module.create_jit_execution_engine(OptimizationLevel::Aggressive);
        if let Ok(execution_engine) = rs_engine {
            let mut func_generator = Self {
                context,
                module,
                builder: context.create_builder(),
                execution_engine,
                type_wrapper: None,
            };
            let i64_type = func_generator.context.i64_type();
            let i16_type = func_generator.context.i16_type();
            let i8_type = func_generator.context.i8_type();
            let ret_val_type = func_generator
                .context
                .struct_type(&[i8_type.into(), i64_type.into()], false);
            let fn_type =
                ret_val_type.fn_type(&[i64_type.into(), i16_type.into(), i16_type.into()], false);
            reg_rust_fn(
                &func_generator,
                "fetch_i64",
                fn_type,
                fetch_column_i64 as usize,
            );
            reg_rust_fn(
                &func_generator,
                "fetch_f64",
                fn_type,
                fetch_column_f64 as usize,
            );
            func_generator.type_wrapper.replace(ret_val_type);
            func_generator
        } else {
            panic!("{:?}", rs_engine.err().unwrap());
        }
    }

    pub(crate) fn compile<T>(&self, name: &str) -> Option<JitFunction<'_, T>>
    where
        T: UnsafeFunctionPointer,
    {
        log::info!("try to compile func: {name}");
        unsafe {
            let rs = self.execution_engine.get_function(name);
            if let Ok(func) = rs {
                Some(func)
            } else {
                log::warn!("hit error: {:?}", rs.err().unwrap());
                None
            }
        }
    }

    pub(crate) fn get_ret_val_type(&self) -> &StructType<'_> {
        self.type_wrapper.as_ref().unwrap()
    }
}

pub(crate) fn init_func_generator() {
    unsafe {
        PTR_LLVM_CTX_STORE = def_global_ptr(Context::create());
    }
}

unsafe extern "C" fn fetch_column_i64(data_ptr: u64, record_id: u16, column_id: u16) -> RetVal {
    if let Some(column) = Record::get_column(record_id, column_id) {
        let v: i64 = fetch_ptr(unsafe { (data_ptr as *const u8).add(column.offset()) });
        RetVal::new(T_I64, v.cast_unsigned())
    } else {
        log::warn!("cannot found the column({column_id}) in record({record_id})");
        RetVal::new(T_ERR, 0)
    }
}

unsafe extern "C" fn fetch_column_f64(data_ptr: u64, record_id: u16, column_id: u16) -> RetVal {
    if let Some(column) = Record::get_column(record_id, column_id) {
        let v: f64 = fetch_ptr(unsafe { (data_ptr as *const u8).add(column.offset()) });
        RetVal::new(T_F64, v.to_bits())
    } else {
        log::warn!("cannot found the column({column_id}) in record({record_id})");
        RetVal::new(T_ERR, 0)
    }
}

fn get_ctx() -> &'static mut Context {
    get_global_mut(unsafe { PTR_LLVM_CTX_STORE })
}

fn reg_rust_fn<'ctx>(
    func_generator: &FuncGenerator<'ctx>,
    name: &str,
    fn_type: FunctionType<'ctx>,
    fn_addr: usize,
) {
    let rust_fn = func_generator.module.add_function(name, fn_type, None);
    func_generator
        .execution_engine
        .add_global_mapping(&rust_fn, fn_addr);
}
