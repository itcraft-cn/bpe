use crate::{aux::fetch_ptr, data::Record};
use inkwell::{
    builder::Builder,
    context::Context,
    execution_engine::{ExecutionEngine, JitFunction, UnsafeFunctionPointer},
    module::Module,
    types::FunctionType,
    OptimizationLevel,
};
use std::{ptr, sync::Mutex};

static LLVM_CTX_STORE: Mutex<u64> = Mutex::new(0);

pub(crate) type FetchU64Func = unsafe extern "C" fn(u64, u16, u16) -> u64;
pub(crate) type FetchI64Func = unsafe extern "C" fn(u64, u16, u16) -> i64;
pub(crate) type FetchF64Func = unsafe extern "C" fn(u64, u16, u16) -> f64;

#[derive(Debug)]
pub(crate) struct FuncGenerator<'ctx> {
    pub(crate) context: &'ctx Context,
    pub(crate) module: Module<'ctx>,
    pub(crate) builder: Builder<'ctx>,
    pub(crate) execution_engine: ExecutionEngine<'ctx>,
}
impl FuncGenerator<'_> {
    pub(crate) fn new() -> Self {
        let context = get_ctx();
        let module = context.create_module("llvm_jit");
        let rs_engine = module.create_jit_execution_engine(OptimizationLevel::Aggressive);
        if let Ok(execution_engine) = rs_engine {
            let func_generator = Self {
                context,
                module,
                builder: context.create_builder(),
                execution_engine,
            };
            func_generator.reg_func_u64("fetch_u64", fetch_column_u64);
            func_generator.reg_func_i64("fetch_i64", fetch_column_i64);
            func_generator.reg_func_f64("fetch_f64", fetch_column_f64);
            func_generator
        } else {
            panic!("{:?}", rs_engine.err().unwrap());
        }
    }

    fn reg_func_u64(&self, name: &str, f: FetchU64Func) {
        let i64_type = self.context.i64_type();
        let i16_type = self.context.i16_type();

        let fn_type = i64_type.fn_type(&[i64_type.into(), i16_type.into(), i16_type.into()], false);

        reg_rust_fn(self, name, fn_type, f as usize);
    }

    fn reg_func_i64(&self, name: &str, f: FetchI64Func) {
        let i64_type = self.context.i64_type();
        let i16_type = self.context.i16_type();

        let fn_type = i64_type.fn_type(&[i64_type.into(), i16_type.into(), i16_type.into()], false);

        reg_rust_fn(self, name, fn_type, f as usize);
    }

    fn reg_func_f64(&self, name: &str, f: FetchF64Func) {
        let f64_type = self.context.f64_type();
        let i64_type = self.context.i64_type();
        let i16_type = self.context.i16_type();

        let fn_type = f64_type.fn_type(&[i64_type.into(), i16_type.into(), i16_type.into()], false);

        reg_rust_fn(self, name, fn_type, f as usize);
    }

    pub(crate) fn compile<T>(&self, name: &str) -> Option<JitFunction<T>>
    where
        T: UnsafeFunctionPointer,
    {
        log::info!("try to compile func: {}", name);
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
}

pub(crate) fn init_func_generator() {
    let boxed_context = Box::new(Context::create());
    let static_context = Box::leak::<'static>(boxed_context);
    let ctx_ptr = ptr::addr_of_mut!(*static_context);
    let lock_rs = LLVM_CTX_STORE.lock();
    if let Ok(mut guard) = lock_rs {
        *guard = ctx_ptr as u64;
    } else {
        panic!("failed to init func generator, {}", lock_rs.err().unwrap());
    }
}

unsafe extern "C" fn fetch_column_u64(data_ptr: u64, record_id: u16, column_id: u16) -> u64 {
    if let Some(column) = Record::get_column(record_id, column_id) {
        return fetch_ptr(unsafe { (data_ptr as *const u8).add(column.offset()) });
    }
    0
}

unsafe extern "C" fn fetch_column_i64(data_ptr: u64, record_id: u16, column_id: u16) -> i64 {
    if let Some(column) = Record::get_column(record_id, column_id) {
        return fetch_ptr(unsafe { (data_ptr as *const u8).add(column.offset()) });
    }
    0
}

unsafe extern "C" fn fetch_column_f64(data_ptr: u64, record_id: u16, column_id: u16) -> f64 {
    if let Some(column) = Record::get_column(record_id, column_id) {
        return fetch_ptr(unsafe { (data_ptr as *const u8).add(column.offset()) });
    }
    0_f64
}

fn get_ctx() -> &'static mut Context {
    let lock_rs = LLVM_CTX_STORE.lock();
    if let Ok(guard) = lock_rs {
        if *guard == 0 {
            panic!("failed to get llvm context, null pointer");
        } else {
            let ctx_ptr = *guard as *mut Context;
            unsafe { &mut *ctx_ptr }
        }
    } else {
        panic!("failed to get llvm context, {}", lock_rs.err().unwrap());
    }
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
