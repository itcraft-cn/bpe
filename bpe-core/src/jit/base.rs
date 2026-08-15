use crate::{
    jit::aux::{
        fetch_column_f64, fetch_column_i64, int2float, jit_abs, jit_add, jit_ceil, jit_div,
        jit_exp, jit_floor, jit_greatest, jit_least, jit_ln, jit_log10, jit_mod, jit_mul,
        jit_pow, jit_dim_get, jit_dim_has, jit_round, jit_sign, jit_sqrt, jit_sub, jit_to_double,
        jit_to_long, jit_trunc, llvm_log_float, llvm_log_int,
    },
};
use globalvar::{def_global_ptr, get_global_mut};
use inkwell::{
    builder::Builder,
    context::Context,
    execution_engine::{ExecutionEngine, JitFunction, UnsafeFunctionPointer},
    module::Module,
    types::{FunctionType, IntType, StructType},
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
    /// Creates a new BinaryExpression with the specified left and right value types and values.
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
    /// Creates a new function generator with LLVM context, module, builder, and execution engine.
    /// Initializes the JIT environment and registers Rust functions for use in generated code.
    pub(crate) fn new() -> Self {
        let context = get_ctx(); // Get the LLVM context
        let module = context.create_module("llvm_jit"); // Create a new LLVM module
        let rs_engine = module.create_jit_execution_engine(OptimizationLevel::Aggressive); // Create execution engine
        if let Ok(execution_engine) = rs_engine {
            let mut func_generator = Self {
                context,
                module,
                builder: context.create_builder(), // Create LLVM builder
                execution_engine,
                type_wrapper: None,
            };
            let i64_type = func_generator.context.i64_type(); // Get i64 type
            let i16_type = func_generator.context.i16_type(); // Get i16 type
            let f64_type = func_generator.context.f64_type(); // Get f64 type
            let void_type = func_generator.context.void_type(); // Get void type

            // Define return type for fetch functions (struct with two i64 values)
            let fetch_ret_val_type = func_generator
                .context
                .struct_type(&[i64_type.into(), i64_type.into()], false);
            // Define function type for fetch functions (u64ptr, u16, u16 -> struct)
            let fetch_fn_type = fetch_ret_val_type
                .fn_type(&[i64_type.into(), i16_type.into(), i16_type.into()], false);

            // Define function type for int-to-float conversion (i64, i64 -> f64)
            let i2f_fn_type = f64_type.fn_type(&[i64_type.into(), i64_type.into()], false);

            // Define function type for logging function (u64, u64, u16 -> void)
            let log_int_fn_type =
                void_type.fn_type(&[i64_type.into(), i64_type.into(), i16_type.into()], false);

            // Define function type for logging function (f64, u64, u16 -> void)
            let log_float_fn_type =
                void_type.fn_type(&[f64_type.into(), i64_type.into(), i16_type.into()], false);

            // Register Rust functions for use in generated LLVM code
            reg_rust_fn(
                &func_generator,
                "fetch_i64", // Register function to fetch i64 values
                fetch_fn_type,
                fetch_column_i64 as *const () as usize,
            );
            reg_rust_fn(
                &func_generator,
                "fetch_f64", // Register function to fetch f64 values
                fetch_fn_type,
                fetch_column_f64 as *const () as usize,
            );
            reg_rust_fn(
                &func_generator,
                "i2f", // Register function for int-to-float conversion
                i2f_fn_type,
                int2float as *const () as usize,
            );
            reg_rust_fn(
                &func_generator,
                "logint", // Register logging function
                log_int_fn_type,
                llvm_log_int as *const () as usize,
            );
            reg_rust_fn(
                &func_generator,
                "logfloat", // Register logging function
                log_float_fn_type,
                llvm_log_float as *const () as usize,
            );
            // Register scalar functions so WHERE clauses can call them via `_name(...)`
            register_scalar_funcs(&func_generator, fetch_ret_val_type, i64_type);
            func_generator.type_wrapper.replace(fetch_ret_val_type); // Store the return type
            func_generator
        } else {
            panic!("{:?}", rs_engine.err().unwrap()); // Panic if engine creation failed
        }
    }

    /// Compiles a function with the given name from the JIT module and returns a handle to it.
    /// Returns Some(JitFunction) if successful, or None if the function could not be found.
    pub(crate) fn compile<T>(&self, name: &str) -> Option<JitFunction<'_, T>>
    where
        T: UnsafeFunctionPointer,
    {
        unsafe {
            let rs = self.execution_engine.get_function(name); // Get function by name
            if let Ok(func) = rs {
                Some(func) // Return the function if found
            } else {
                log::warn!("hit error: {:?}", rs.err().unwrap()); // Log error if function not found
                None
            }
        }
    }

    /// Returns the struct type used for return values from fetch operations.
    pub(crate) fn get_ret_val_type(&self) -> &StructType<'_> {
        self.type_wrapper.as_ref().unwrap() // Return the stored return value type
    }
}

#[derive(Debug)]
pub(crate) struct GenContext<'ctx> {
    pub(crate) func_generator: &'ctx FuncGenerator<'ctx>,
    pub(crate) param_u64ptr: IntValue<'ctx>,
    pub(crate) i2f_func: FunctionValue<'ctx>,
    pub(crate) _log_int_func: FunctionValue<'ctx>,
    pub(crate) _log_float_func: FunctionValue<'ctx>,
}
impl<'ctx> GenContext<'ctx> {
    /// Creates a new generation context with the specified function generator and LLVM function values.
    pub(crate) fn new(
        func_generator: &'ctx FuncGenerator<'ctx>,
        param_u64ptr: IntValue<'ctx>,
        i2f_func: FunctionValue<'ctx>,
        log_int_func: FunctionValue<'ctx>,
        log_float_func: FunctionValue<'ctx>,
    ) -> Self {
        Self {
            func_generator,
            param_u64ptr,
            i2f_func,
            _log_int_func: log_int_func,
            _log_float_func: log_float_func,
        }
    }
}

/// Initializes the function generator by creating an LLVM context and storing it globally.
pub(crate) fn init_func_generator() {
    unsafe {
        PTR_LLVM_CTX_STORE = def_global_ptr(Context::create()); // Create and store LLVM context globally
    }
}

/// Gets the global LLVM context that was initialized earlier.
fn get_ctx() -> &'static mut Context {
    get_global_mut(unsafe { PTR_LLVM_CTX_STORE }) // Retrieve the stored LLVM context
}

/// Registers a Rust function in the LLVM module so it can be called from generated code.
fn reg_rust_fn<'ctx>(
    func_generator: &FuncGenerator<'ctx>,
    name: &str,
    fn_type: FunctionType<'ctx>,
    fn_addr: usize,
) {
    let rust_fn = func_generator.module.add_function(name, fn_type, None); // Add function to module
    func_generator
        .execution_engine
        .add_global_mapping(&rust_fn, fn_addr); // Map the function address in the execution engine
}

/// Registers the scalar functions (unary: `(t1,v1) -> RetVal`, binary: `(t1,v1,t2,v2) -> RetVal`)
/// into the LLVM module under `func_{name}`, so JIT-compiled WHERE clauses can call
/// `_name(...)` the same way SELECT fields do in the interpreter.
fn register_scalar_funcs<'ctx>(
    func_generator: &FuncGenerator<'ctx>,
    ret_val_type: StructType<'ctx>,
    i64_type: IntType<'ctx>,
) {
    let unary_fn_type = ret_val_type.fn_type(
        &[i64_type.into(), i64_type.into()],
        false,
    );
    let binary_fn_type = ret_val_type.fn_type(
        &[
            i64_type.into(),
            i64_type.into(),
            i64_type.into(),
            i64_type.into(),
        ],
        false,
    );
    macro_rules! reg {
        ($name:literal, $fn_addr:expr) => {
            reg_rust_fn(
                func_generator,
                $name,
                unary_fn_type,
                $fn_addr as *const () as usize,
            )
        };
    }
    macro_rules! reg2 {
        ($name:literal, $fn_addr:expr) => {
            reg_rust_fn(
                func_generator,
                $name,
                binary_fn_type,
                $fn_addr as *const () as usize,
            )
        };
    }
    reg!("func_abs", jit_abs);
    reg!("func_ceil", jit_ceil);
    reg!("func_floor", jit_floor);
    reg!("func_round", jit_round);
    reg!("func_trunc", jit_trunc);
    reg!("func_sign", jit_sign);
    reg!("func_sqrt", jit_sqrt);
    reg!("func_exp", jit_exp);
    reg!("func_ln", jit_ln);
    reg!("func_log10", jit_log10);
    reg!("func_to_long", jit_to_long);
    reg!("func_to_double", jit_to_double);
    reg2!("func_add", jit_add);
    reg2!("func_sub", jit_sub);
    reg2!("func_mul", jit_mul);
    reg2!("func_div", jit_div);
    reg2!("func_mod", jit_mod);
    reg2!("func_pow", jit_pow);
    reg2!("func_greatest", jit_greatest);
    reg2!("func_least", jit_least);
    reg2!("func_dim_has", jit_dim_has);
    reg2!("func_dim_get", jit_dim_get);
}
