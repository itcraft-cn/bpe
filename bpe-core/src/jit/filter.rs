use crate::{
    data::{ColumnType, Record},
    jit::{
        base::{BinaryExpression, FuncGenerator, GenContext, LogicOpFnType},
        consts::{B_TRUE, T_B64, T_F64, T_I64},
    },
    sql::base::FilterFunc,
};
use inkwell::{
    basic_block::BasicBlock,
    execution_engine::JitFunction,
    types::IntType,
    values::{BasicValueEnum, FloatValue, IntValue, StructValue},
    FloatPredicate, IntPredicate,
};
use sql_parse::{BinaryOperator, Expression, IdentifierPart};
use std::{
    ops::Range,
    sync::atomic::{AtomicU64, Ordering},
};

/// Generates a JIT-compiled filter function for a SQL SELECT statement based on the WHERE clause.
/// This function creates an LLVM function that evaluates the filter condition and returns a boolean.
pub(crate) fn gen_select_filter_func<'ctx>(
    func_generator: &'ctx FuncGenerator<'ctx>, // Function generator to create LLVM code
    where_: &Option<(Expression<'_>, Range<usize>)>, // Optional WHERE clause expression
    record: &Record,                           // Record definition for column access
    issues: &mut Vec<String>,                  // Vector to collect error messages
) -> Result<JitFunction<'ctx, FilterFunc>, String> {
    let i64_type = func_generator.context.i64_type(); // Get i64 type for LLVM
    let bool_type = func_generator.context.bool_type(); // Get boolean type for LLVM

    // Define function type: takes u64 parameter, returns boolean
    let fn_type = bool_type.fn_type(&[i64_type.into()], true);
    let func_name = &format!("{}_{}", "record_filter", record.id()); // Create unique function name
    let func = func_generator.module.add_function(func_name, fn_type, None); // Add function to module
    let block = func_generator.context.append_basic_block(func, "entry"); // Create entry block
    func_generator.builder.position_at_end(block); // Position builder at end of block
    let mut walker = AtomicU64::new(0); // Initialize atomic walker for unique names
    let ret;
    if let Some(where_part) = where_ {
        // Extract the u64 pointer parameter from the function
        let param_u64ptr = func.get_nth_param(0).unwrap().into_int_value();
        // Get references to helper functions in the module
        let i2f_func = func_generator.module.get_function("i2f").unwrap();
        let log_int_func = func_generator.module.get_function("logint").unwrap();
        let log_float_func = func_generator.module.get_function("logfloat").unwrap();
        // Create generation context with the function generator and helper functions
        let context = GenContext::new(
            func_generator,
            param_u64ptr,
            i2f_func,
            log_int_func,
            log_float_func,
        );
        // Parse the WHERE expression into an LLVM value
        let opt_val = parse_exp(&context, &where_part.0, record, issues, &mut walker);
        if let Some(val) = opt_val {
            // Extract type field from the returned struct
            let type_val = val.get_field_at_index(0).unwrap().into_int_value();
            let bool_val = i64_type.const_int(T_B64, true); // Boolean type constant

            // Check if the type is boolean
            let type_check_ret = func_generator
                .builder
                .build_int_compare(IntPredicate::EQ, type_val, bool_val, "type_check_status")
                .unwrap();

            // Extract value field from the returned struct
            let ret_val = val.get_field_at_index(1).unwrap().into_int_value();
            let true_val = i64_type.const_int(B_TRUE, true); // True value constant

            // Check if the value is true
            let val_check_ret = func_generator
                .builder
                .build_int_compare(IntPredicate::EQ, ret_val, true_val, "val_check_status")
                .unwrap();

            // Combine type check and value check with AND operation
            ret = func_generator
                .builder
                .build_and(type_check_ret, val_check_ret, "ret_status")
                .unwrap();
        } else {
            return Err("failed to gen filter function".to_string()); // Return error if parsing failed
        }
    } else {
        ret = bool_type.const_int(B_TRUE, true); // Return true if no WHERE clause
    }
    let _ = func_generator.builder.build_return(Some(&ret)); // Build return statement
    let opt_filter_func = func_generator.compile::<FilterFunc>(func_name); // Compile the function
    if let Some(filter_func) = opt_filter_func {
        Ok(filter_func) // Return the compiled function
    } else {
        Err("failed to compile filter function".to_string()) // Return error if compilation failed
    }
}

/// Parses a SQL expression into an LLVM value by recursively processing the expression tree.
/// Returns Some(StructValue) if successful, or None if the expression could not be parsed.
fn parse_exp<'ctx>(
    context: &GenContext<'ctx>, // Generation context with LLVM builder and types
    expr: &Expression<'_>,      // The SQL expression to parse
    record: &Record,            // Record definition for column access
    issues: &mut Vec<String>,   // Vector to collect error messages
    walker: &mut AtomicU64,     // Atomic counter for generating unique names
) -> Option<StructValue<'ctx>> {
    match expr {
        Expression::Binary {
            op,
            op_span: _,
            lhs,
            rhs,
        } => {
            // Parse binary expression by first parsing left and right sides
            let rs = parse_binary_exp(context, record, issues, walker, lhs, rhs);
            if let Ok(bin_exp) = rs {
                // Calculate the binary operation using the parsed expression
                bin_op_calc(context, expr, issues, walker, op, &bin_exp)
            } else {
                // Add error message if binary expression parsing failed
                issues.push(format!("unsupported expression: {expr:?}, {:?}", rs.err()));
                None
            }
        }
        Expression::Identifier(id_vec) => parse_identifier(context, record, issues, id_vec), // Parse column identifier
        Expression::Integer(group) => parse_val(context, T_I64, group.0), // Parse integer literal
        Expression::Float(group) => parse_val(context, T_F64, group.0.to_bits()), // Parse float literal
        _ => parse_unsupported(expr, issues), // Handle unsupported expression types
    }
}

/// Parses a binary expression by recursively parsing the left and right operands.
/// Returns Ok(BinaryExpression) if both operands were parsed successfully, or an error if parsing failed.
fn parse_binary_exp<'ctx>(
    context: &GenContext<'ctx>, // Generation context with LLVM builder and types
    record: &Record,            // Record definition for column access
    issues: &mut Vec<String>,   // Vector to collect error messages
    walker: &mut AtomicU64,     // Atomic counter for generating unique names
    lhs: &Expression<'_>,       // Left-hand side of the binary expression
    rhs: &Expression<'_>,       // Right-hand side of the binary expression
) -> Result<BinaryExpression<'ctx>, String> {
    // Parse left and right expressions recursively
    let opt_l_ret = parse_exp(context, lhs, record, issues, walker);
    let opt_r_ret = parse_exp(context, rhs, record, issues, walker);
    if opt_l_ret.is_some() && opt_r_ret.is_some() {
        let l_ret = unwrap_opt(opt_l_ret); // Unwrap left result
        let r_ret = unwrap_opt(opt_r_ret); // Unwrap right result

        // Extract type and value fields from both results
        let l_val_type = l_ret.get_field_at_index(0).unwrap(); // Left value type
        let r_val_type = r_ret.get_field_at_index(0).unwrap(); // Right value type
        let l_val = l_ret.get_field_at_index(1).unwrap(); // Left value
        let r_val = r_ret.get_field_at_index(1).unwrap(); // Right value

        // Create a new BinaryExpression with the extracted values
        Ok(BinaryExpression::new(l_val_type, r_val_type, l_val, r_val))
    } else if opt_l_ret.is_none() {
        Err("failed to parse left expression".to_string()) // Error if left expression failed
    } else {
        Err("failed to parse right expression".to_string()) // Error if right expression failed
    }
}

fn unwrap_opt(opt: Option<StructValue<'_>>) -> StructValue<'_> {
    match opt {
        Some(v) => v,
        _ => panic!(),
    }
}

/// Parses a column identifier (e.g., "column_name" or "table.column_name") into an LLVM value.
/// Currently only supports simple column names without table prefixes.
fn parse_identifier<'ctx>(
    context: &GenContext<'ctx>, // Generation context with LLVM builder and types
    record: &Record,            // Record definition for column access
    issues: &mut Vec<String>,   // Vector to collect error messages
    id_vec: &Vec<IdentifierPart<'_>>, // Vector of identifier parts
) -> Option<StructValue<'ctx>> {
    let len = id_vec.len(); // Get the number of identifier parts
    if len == 1 {
        // Simple column name: column_name
        let idp = id_vec.first().unwrap(); // Get the identifier part
        gen_call_fetch_column(context, idp, record, issues) // Generate fetch call
    } else if len == 2 {
        // Column with table prefix: table.column_name (currently not fully supported)
        let _tab_id_part = id_vec.first().unwrap(); // Get table part (currently unused)
        let idp = id_vec.get(1).unwrap(); // Get column part
        gen_call_fetch_column(context, idp, record, issues) // Generate fetch call
    } else {
        // More than 2 parts are not supported
        issues.push(format!("unsupported id: {id_vec:?}")); // Add error message
        None
    }
}

/// Creates an LLVM struct representing a literal value with its type and value.
/// This is used for integer and float literals in expressions.
fn parse_val<'ctx>(
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

/// Handles unsupported expression types by adding an error message to the issues vector.
/// Returns None to indicate that the expression could not be parsed.
fn parse_unsupported<'ctx>(
    expr: &Expression<'_>,    // The unsupported expression
    issues: &mut Vec<String>, // Vector to collect error messages
) -> Option<StructValue<'ctx>> {
    let issue_desc = match expr {
        Expression::String(str) => format!("unsupported String: {str:?}"), // String literals not supported
        Expression::Function(f, expr_vec, _) => {
            // Function calls not supported
            format!("unsupported func: {f:?}, expr_vec: {expr_vec:?}")
        }
        _ => format!("unsupported expr condition: {expr:?}"), // Other unsupported expressions
    };
    issues.push(issue_desc); // Add the error message to the issues vector
    None // Return None to indicate failure
}

/// Calculates the result of a binary operation (OR, AND, comparison operators) by generating
/// appropriate LLVM code for the operation.
fn bin_op_calc<'ctx>(
    context: &GenContext<'ctx>, // Generation context with LLVM builder and types
    expr: &Expression<'_>,      // The expression being processed
    issues: &mut Vec<String>,   // Vector to collect error messages
    walker: &mut AtomicU64,     // Atomic counter for generating unique names
    op: &BinaryOperator,        // The binary operator to apply
    bin_exp: &BinaryExpression<'ctx>, // The left and right operands
) -> Option<StructValue<'ctx>> {
    match op {
        BinaryOperator::Or => logic_op(context, walker, bin_exp, |builder, v1, v2, walker| {
            // Handle logical OR operation
            let w = walker.fetch_add(1, Ordering::SeqCst); // Generate unique name
            builder.build_or(v1, v2, &format!("val_{w}_or")).unwrap() // Build OR operation
        }),
        BinaryOperator::And => logic_op(context, walker, bin_exp, |builder, v1, v2, walker| {
            // Handle logical AND operation
            let w = walker.fetch_add(1, Ordering::SeqCst); // Generate unique name
            builder.build_and(v1, v2, &format!("val_{w}_and")).unwrap() // Build AND operation
        }),
        BinaryOperator::Eq
        | BinaryOperator::GtEq
        | BinaryOperator::Gt
        | BinaryOperator::LtEq
        | BinaryOperator::Lt
        | BinaryOperator::Neq => {
            // Handle comparison operations (==, >=, >, <=, <, !=)
            let (matched, int_op, float_op, name) = match op {
                BinaryOperator::Eq => (true, IntPredicate::EQ, FloatPredicate::OEQ, "eq"), // Equal
                BinaryOperator::GtEq => (true, IntPredicate::SGE, FloatPredicate::OGE, "gteq"), // Greater or equal
                BinaryOperator::Gt => (true, IntPredicate::SGT, FloatPredicate::OGT, "gt"), // Greater than
                BinaryOperator::LtEq => (true, IntPredicate::SLE, FloatPredicate::OLE, "lteq"), // Less or equal
                BinaryOperator::Lt => (true, IntPredicate::SLT, FloatPredicate::OLT, "lt"), // Less than
                BinaryOperator::Neq => (true, IntPredicate::NE, FloatPredicate::ONE, "neq"), // Not equal
                _ => (false, IntPredicate::EQ, FloatPredicate::OEQ, "!!!op"), // Fallback case
            };
            if matched {
                // Perform the comparison operation (int or float)
                logic_compare(context, walker, bin_exp, int_op, float_op, name)
            } else {
                issues.push(format!("unsupported op: {op:?}")); // Add error for unsupported op
                None
            }
        }
        _ => {
            // Handle any other unsupported operators
            issues.push(format!("unsupported expr condition: {expr:?}")); // Add error message
            None
        }
    }
}

/// Performs a logical operation (OR, AND) on two operands by calling the provided function.
/// Returns the result as an LLVM struct with type and value fields.
fn logic_op<'ctx>(
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
fn logic_compare<'ctx>(
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
fn check_is_float_cmp<'ctx>(
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
fn build_float_cmp<'ctx>(
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
    let l_float_val = convert_int2float(context, "l", l_val_type, l_val); // Convert left value
    let r_float_val = convert_int2float(context, "r", r_val_type, r_val); // Convert right value

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
fn convert_int2float<'ctx>(
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
fn build_int_cmp<'ctx>(
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
fn ret_phi_int_val<'ctx>(
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

/// Generates LLVM code to call a column fetch function based on the column identifier.
/// Returns the fetched value as an LLVM struct with type and value fields.
fn gen_call_fetch_column<'ctx>(
    context: &GenContext<'ctx>, // Generation context with LLVM builder and types
    idp: &IdentifierPart<'_>,   // Identifier part containing the column name
    record: &Record,            // Record definition for column access
    issues: &mut Vec<String>,   // Vector to collect error messages
) -> Option<StructValue<'ctx>> {
    match idp {
        IdentifierPart::Name(id) => {
            let name = id.as_str(); // Get the column name as a string
            let column_id = *record.column_id(name).unwrap(); // Get the column ID from the record
            let column = record.column(column_id); // Get the column definition
            let i16_type = context.func_generator.context.i16_type(); // Get i16 type for IDs

            // Create constants for record ID and column ID
            let param_record_id = i16_type.const_int(record.id() as u64, true);
            let param_column_id = i16_type.const_int(column_id as u64, true);
            // Select the appropriate fetch function based on column data type
            let fetch_val_func = match column.data_type() {
                ColumnType::Long => context.func_generator.module.get_function("fetch_i64"), // For integer columns
                ColumnType::Double => context.func_generator.module.get_function("fetch_f64"), // For float columns
            }
            .unwrap();

            // Build a call to the fetch function with parameters: data pointer, record ID, column ID
            let call_site_value = context
                .func_generator
                .builder
                .build_call(
                    fetch_val_func,
                    &[
                        context.param_u64ptr.into(), // Data pointer parameter
                        param_record_id.into(),      // Record ID parameter
                        param_column_id.into(),      // Column ID parameter
                    ],
                    "ret", // Name for the call result
                )
                .unwrap();
            let val_enum = call_site_value.try_as_basic_value().unwrap_basic(); // Get the return value
            match val_enum {
                BasicValueEnum::StructValue(struct_value) => {
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
                    let ret_val_type = context.func_generator.get_ret_val_type(); // Get return type

                    // Create a struct with type and value fields
                    Some(ret_val_type.const_named_struct(&[data_type.into(), data.into()]))
                }
                _ => {
                    // Add error if the return value is not a struct
                    issues.push(format!("unsupported BasicValueEnum: {val_enum:?}"));
                    None
                }
            }
        }
        _ => {
            // Add error for unsupported identifier types
            issues.push(format!("unsupported id: {idp:?}"));
            None
        }
    }
}
