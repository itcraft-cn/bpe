use crate::jit::fnaux::{fetch_column_f64, fetch_column_i64, int2float, llvm_log};
use globalvar::{def_global_ptr, get_global_mut};
use inkwell::{
    builder::Builder,
    context::Context,
    execution_engine::{ExecutionEngine, JitFunction, UnsafeFunctionPointer},
    module::Module,
    types::{FunctionType, StructType},
    values::{BasicValueEnum, FunctionValue, IntValue},
    OptimizationLevel,
};
use std::sync::atomic::AtomicU64;

static mut PTR_LLVM_CTX_STORE: u64 = 0;

pub(crate) type LogicOpFnType<'ctx> =
    fn(&Builder<'ctx>, IntValue<'ctx>, IntValue<'ctx>, &mut AtomicU64) -> IntValue<'ctx>;

#[derive(Debug)]
pub(crate) struct BinaryExpression<'a> {
    pub(crate) l_val_type: BasicValueEnum<'a>,
    pub(crate) r_val_type: BasicValueEnum<'a>,
    pub(crate) l_val: BasicValueEnum<'a>,
    pub(crate) r_val: BasicValueEnum<'a>,
}
impl<'a> BinaryExpression<'a> {
    pub(crate) fn new(
        l_val_type: BasicValueEnum<'a>,
        r_val_type: BasicValueEnum<'a>,
        l_val: BasicValueEnum<'a>,
        r_val: BasicValueEnum<'a>,
    ) -> Self {
        Self {
            l_val_type,
            r_val_type,
            l_val,
            r_val,
        }
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
            let fetch_ret_val_type = func_generator
                .context
                .struct_type(&[i64_type.into(), i64_type.into()], false);
            let fetch_fn_type = fetch_ret_val_type
                .fn_type(&[i64_type.into(), i16_type.into(), i16_type.into()], false);
            let i2f_ret_val_type = func_generator.context.f64_type();
            let i2f_fn_type = i2f_ret_val_type.fn_type(&[i64_type.into(), i64_type.into()], false);
            let log_ret_val_type = func_generator.context.void_type();
            let log_fn_type = log_ret_val_type
                .fn_type(&[i64_type.into(), i64_type.into(), i16_type.into()], false);
            reg_rust_fn(
                &func_generator,
                "fetch_i64",
                fetch_fn_type,
                fetch_column_i64 as *const () as usize,
            );
            reg_rust_fn(
                &func_generator,
                "fetch_f64",
                fetch_fn_type,
                fetch_column_f64 as *const () as usize,
            );
            reg_rust_fn(
                &func_generator,
                "i2f",
                i2f_fn_type,
                int2float as *const () as usize,
            );
            reg_rust_fn(
                &func_generator,
                "log",
                log_fn_type,
                llvm_log as *const () as usize,
            );
            func_generator.type_wrapper.replace(fetch_ret_val_type);
            func_generator
        } else {
            panic!("{:?}", rs_engine.err().unwrap());
        }
    }

    pub(crate) fn compile<T>(&self, name: &str) -> Option<JitFunction<'_, T>>
    where
        T: UnsafeFunctionPointer,
    {
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

#[derive(Debug)]
pub(crate) struct GenContext<'ctx> {
    pub(crate) func_generator: &'ctx FuncGenerator<'ctx>,
    pub(crate) param_u64ptr: IntValue<'ctx>,
    pub(crate) i2f_func: FunctionValue<'ctx>,
    pub(crate) _log_func: FunctionValue<'ctx>,
}
impl<'ctx> GenContext<'ctx> {
    pub(crate) fn new(
        func_generator: &'ctx FuncGenerator<'ctx>,
        param_u64ptr: IntValue<'ctx>,
        i2f_func: FunctionValue<'ctx>,
        log_func: FunctionValue<'ctx>,
    ) -> Self {
        Self {
            func_generator,
            param_u64ptr,
            i2f_func,
            _log_func: log_func,
        }
    }
}

pub(crate) fn init_func_generator() {
    unsafe {
        PTR_LLVM_CTX_STORE = def_global_ptr(Context::create());
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
