use crate::{
    data::{Column, ColumnType},
    jit::{
        base::{BinaryExpression, GenContext, LogicOpFnType},
        consts::{T_B64, T_F64},
    },
};
use inkwell::{
    basic_block::BasicBlock,
    types::IntType,
    values::{FloatValue, FunctionValue, IntValue, StructValue},
    FloatPredicate, IntPredicate,
};
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) fn unwrap_opt(opt: Option<StructValue<'_>>) -> StructValue<'_> {
    match opt {
        Some(v) => v,
        _ => panic!(),
    }
}

/// Creates an LLVM struct representing a literal value with its type and value.
/// This is used for integer and float literals in expressions.
pub(crate) fn parse_val<'ctx>(
    context: &GenContext<'ctx>, // Generation context with LLVM builder and types
    val_type: u64,              // Type identifier for the value (T_I64, T_F64, etc.)
    val: u64,                   // The actual value
) -> Option<StructValue<'ctx>> {
    let i64_type = context.func_generator.context.i64_type(); // Get i64 type
    let data_type = i64_type.const_int(val_type, true); // Create type constant
    let data = i64_type.const_int(val, true); // Create value constant
    let ret_val_type = context.func_generator.get_ret_val_type(); // Get return value type

    // Create a struct with type and value fields
    Some(ret_val_type.const_named_struct(&[data_type.into(), data.into()]))
}

/// Performs a logical operation (OR, AND) on two operands by calling the provided function.
/// Returns the result as an LLVM struct with type and value fields.
pub(crate) fn logic_op<'ctx>(
    context: &GenContext<'ctx>, // Generation context with LLVM builder and types
    walker: &mut AtomicU64,     // Atomic counter for generating unique names
    bin_exp: &BinaryExpression<'ctx>, // The binary expression with left and right values
    f: LogicOpFnType<'ctx>,     // Function to perform the logical operation
) -> Option<StructValue<'ctx>> {
    let i64_type = context.func_generator.context.i64_type(); // Get i64 type
    let l_val = bin_exp.l_val.into_int_value(); // Convert left value to int
    let r_val = bin_exp.r_val.into_int_value(); // Convert right value to int
    let ret_type = i64_type.const_int(T_B64, true); // Set return type as boolean
    let ret_val = f(&context.func_generator.builder, l_val, r_val, walker); // Perform the operation

    // Create and return a struct with type and value fields
    Some(
        context
            .func_generator
            .get_ret_val_type()
            .const_named_struct(&[ret_type.into(), ret_val.into()]),
    )
}

/// Performs a comparison operation (==, !=, <, >, <=, >=) that handles both integer and float values.
/// Uses conditional branching to select between integer and float comparison paths.
pub(crate) fn logic_compare<'ctx>(
    context: &GenContext<'ctx>, // Generation context with LLVM builder and types
    walker: &mut AtomicU64,     // Atomic counter for generating unique names
    bin_exp: &BinaryExpression<'ctx>, // The binary expression with left and right values
    int_op: IntPredicate,       // Integer comparison predicate
    float_op: FloatPredicate,   // Float comparison predicate
    name: &str,                 // Name for the operation
) -> Option<StructValue<'ctx>> {
    let w = walker.fetch_add(1, Ordering::SeqCst); // Generate unique name
    let func_name = &format!("val_{w}_{name}"); // Format function name
    let i64_type = context.func_generator.context.i64_type(); // Get i64 type
    let l_val_type = bin_exp.l_val_type.into_int_value(); // Get left value type
    let r_val_type = bin_exp.r_val_type.into_int_value(); // Get right value type

    let t_f64 = i64_type.const_int(T_F64, true); // Float type constant

    // Check if either operand is a float value
    let is_float = check_is_float_cmp(context, l_val_type, r_val_type, t_f64);

    // Get the current function to append basic blocks to
    let current_block = context.func_generator.builder.get_insert_block().unwrap();
    let current_func = current_block.get_parent().unwrap();

    // Create basic blocks for different comparison paths
    let float_cmp_block = context
        .func_generator
        .context
        .append_basic_block(current_func, &format!("{}_float_cmp", func_name)); // Float comparison block
    let int_cmp_block = context
        .func_generator
        .context
        .append_basic_block(current_func, &format!("{}_int_cmp", func_name)); // Integer comparison block
    let merge_block = context
        .func_generator
        .context
        .append_basic_block(current_func, &format!("{}_merge", func_name)); // Merge block

    // Branch to appropriate comparison block based on whether operands are floats
    context
        .func_generator
        .builder
        .build_conditional_branch(is_float, float_cmp_block, int_cmp_block)
        .unwrap();

    // Build float comparison and get result
    let float_result = build_float_cmp(
        context,
        float_op,
        i64_type,
        bin_exp,
        float_cmp_block,
        merge_block,
    );

    // Build int comparison and get result
    let int_result = build_int_cmp(
        context,
        int_op,
        i64_type,
        bin_exp,
        int_cmp_block,
        merge_block,
    );

    // Position builder at merge block to create PHI node
    context.func_generator.builder.position_at_end(merge_block);

    let ret_type = i64_type.const_int(T_B64, true); // Set return type as boolean

    // Create PHI node to merge results from both paths
    let ret_val = ret_phi_int_val(
        context,
        func_name,
        i64_type,
        float_cmp_block,
        int_cmp_block,
        float_result,
        int_result,
    );

    // Create and return a struct with type and value fields
    Some(
        context
            .func_generator
            .get_ret_val_type()
            .const_named_struct(&[ret_type.into(), ret_val.into()]),
    )
}

/// Checks if either operand in a comparison is a float value by comparing their type tags.
/// Returns an LLVM boolean value indicating whether either operand is a float.
pub(crate) fn check_is_float_cmp<'ctx>(
    context: &GenContext<'ctx>, // Generation context with LLVM builder and types
    l_val_type: IntValue<'ctx>, // Type of the left operand
    r_val_type: IntValue<'ctx>, // Type of the right operand
    t_f64: IntValue<'ctx>,      // Float type constant (T_F64)
) -> IntValue<'ctx> {
    // Check if left operand is a float
    let l_float = context
        .func_generator
        .builder
        .build_int_compare(IntPredicate::EQ, l_val_type, t_f64, "is_l_float")
        .unwrap();
    // Check if right operand is a float
    let r_float = context
        .func_generator
        .builder
        .build_int_compare(IntPredicate::EQ, r_val_type, t_f64, "is_r_float")
        .unwrap();
    // Return true if either operand is a float
    context
        .func_generator
        .builder
        .build_or(l_float, r_float, "is_float")
        .unwrap()
}

/// Builds the LLVM code for float comparison in a dedicated basic block.
/// Converts integer values to floats and performs the comparison.
pub(crate) fn build_float_cmp<'ctx>(
    context: &GenContext<'ctx>, // Generation context with LLVM builder and types
    float_op: FloatPredicate,   // Float comparison predicate (currently unused)
    i64_type: IntType<'ctx>,    // i64 type for LLVM
    bin_exp: &BinaryExpression<'ctx>, // The binary expression with left and right values
    float_cmp_block: BasicBlock<'ctx>, // Basic block for float comparison
    merge_block: BasicBlock<'ctx>, // Basic block to merge results
) -> IntValue<'ctx> {
    // Float comparison block - position builder at the float comparison block
    context
        .func_generator
        .builder
        .position_at_end(float_cmp_block);
    let l_val_type = bin_exp.l_val_type.into_int_value(); // Get left value type
    let r_val_type = bin_exp.r_val_type.into_int_value(); // Get right value type
    let l_val = bin_exp.l_val.into_int_value(); // Get left value
    let r_val = bin_exp.r_val.into_int_value(); // Get right value

    // Convert integer values to floats for comparison
    let l_float_val = convert_int_to_float(context, "l", l_val_type, l_val); // Convert left value
    let r_float_val = convert_int_to_float(context, "r", r_val_type, r_val); // Convert right value

    let float_result_int = context
        .func_generator
        .builder
        .build_float_compare(float_op, l_float_val, r_float_val, "float_cmp")
        .unwrap(); // Boolean result constant

    // Cast the result to the appropriate type
    let float_result = context
        .func_generator
        .builder
        .build_int_cast_sign_flag(float_result_int, i64_type, false, "float_result")
        .unwrap();

    // Branch to merge block after comparison
    context
        .func_generator
        .builder
        .build_unconditional_branch(merge_block)
        .unwrap();
    float_result // Return the comparison result
}

/// Converts an integer value to a float by calling the int-to-float conversion function.
/// This function was registered in the LLVM module during initialization.
pub(crate) fn convert_int_to_float<'ctx>(
    context: &GenContext<'ctx>, // Generation context with LLVM builder and types
    flag: &str, // Flag for naming the conversion (e.g., "l" for left, "r" for right)
    val_type: IntValue<'ctx>, // Type of the value to convert
    val: IntValue<'ctx>, // The integer value to convert
) -> FloatValue<'ctx> {
    // Call the int-to-float conversion function registered in the module
    context
        .func_generator
        .builder
        .build_call(
            context.i2f_func,               // The conversion function
            &[val_type.into(), val.into()], // Parameters: value type and value
            &format!("{flag}_i2f"),         // Name for the call result
        )
        .unwrap()
        .try_as_basic_value()
        .unwrap_basic()
        .into_float_value() // Convert the result to a float value
}

/// Builds the LLVM code for integer comparison in a dedicated basic block.
/// Performs the comparison using the specified integer predicate.
pub(crate) fn build_int_cmp<'ctx>(
    context: &GenContext<'ctx>, // Generation context with LLVM builder and types
    int_op: IntPredicate,       // Integer comparison predicate (e.g., EQ, GT, LT)
    i64_type: IntType<'ctx>,    // i64 type for LLVM
    bin_exp: &BinaryExpression<'ctx>, // The binary expression with left and right values
    int_cmp_block: BasicBlock<'ctx>, // Basic block for integer comparison
    merge_block: BasicBlock<'ctx>, // Basic block to merge results
) -> IntValue<'ctx> {
    // Int comparison block - position builder at the integer comparison block
    context
        .func_generator
        .builder
        .position_at_end(int_cmp_block);
    let l_val = bin_exp.l_val.into_int_value(); // Get left integer value
    let r_val = bin_exp.r_val.into_int_value(); // Get right integer value

    // Perform integer comparison using the specified predicate
    let int_result_int = context
        .func_generator
        .builder
        .build_int_compare(int_op, l_val, r_val, "int_cmp") // Build comparison operation
        .unwrap();

    // Cast the comparison result to the appropriate type
    let int_result = context
        .func_generator
        .builder
        .build_int_cast_sign_flag(int_result_int, i64_type, false, "int_result")
        .unwrap();

    // Branch to merge block after comparison
    context
        .func_generator
        .builder
        .build_unconditional_branch(merge_block)
        .unwrap();
    int_result // Return the comparison result
}

/// Creates a PHI node to merge results from two different execution paths.
/// This is used to combine results from integer and float comparison branches.
pub(crate) fn ret_phi_int_val<'ctx>(
    context: &GenContext<'ctx>, // Generation context with LLVM builder and types
    func_name: &String,         // Name of the function for PHI node naming
    i64_type: IntType<'ctx>,    // i64 type for LLVM
    block1: BasicBlock<'ctx>,   // First basic block (e.g., float comparison block)
    block2: BasicBlock<'ctx>,   // Second basic block (e.g., integer comparison block)
    result1: IntValue<'ctx>,    // Result from first block
    result2: IntValue<'ctx>,    // Result from second block
) -> IntValue<'ctx> {
    // Create PHI node to merge results from both paths
    let phi = context
        .func_generator
        .builder
        .build_phi(i64_type, &format!("{}_phi", func_name)) // Create PHI node with unique name
        .unwrap();
    // Add incoming values from both blocks to the PHI node
    phi.add_incoming(&[(&result1, block1), (&result2, block2)]);
    // Convert the PHI node to an integer value
    phi.as_basic_value().into_int_value()
}

pub(crate) fn choose_fetch_val_fn<'ctx>(
    context: &GenContext<'ctx>,
    column: &Column,
) -> FunctionValue<'ctx> {
    // Select the appropriate fetch function based on column data type
    let fetch_val_func = match column.data_type() {
        ColumnType::Long => context.func_generator.module.get_function("fetch_i64"), // For integer columns
        ColumnType::Double => context.func_generator.module.get_function("fetch_f64"), // For float columns
    }
    .unwrap();
    fetch_val_func
}

pub(crate) fn conv_rust_type_to_llvm_struct<'ctx>(
    context: &GenContext<'ctx>,
    struct_value: StructValue<'ctx>,
) -> Option<StructValue<'ctx>> {
    // Extract type and value fields from the returned struct
    let data_type = context
        .func_generator
        .builder
        .build_extract_value(struct_value, 0, "data_type") // Extract type field
        .unwrap()
        .into_int_value();
    let data = context
        .func_generator
        .builder
        .build_extract_value(struct_value, 1, "data") // Extract value field
        .unwrap()
        .into_int_value();
    let ret_val_type = context.func_generator.get_ret_val_type();
    // Get return type

    // Create a struct with type and value fields
    Some(ret_val_type.const_named_struct(&[data_type.into(), data.into()]))
}
