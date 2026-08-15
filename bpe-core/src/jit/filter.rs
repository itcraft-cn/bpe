use crate::{
    data::Record,
    jit::{
        base::{BinaryExpression, FuncGenerator, GenContext},
        consts::{B_TRUE, T_B64, T_F64, T_I64},
        llvm_misc::{
            choose_fetch_val_fn, conv_rust_type_to_llvm_struct, logic_compare, logic_op, parse_val,
            unwrap_opt,
        },
    },
    sql::base::FilterFunc,
};
use inkwell::{
    execution_engine::JitFunction,
    values::{BasicValueEnum, StructValue},
    FloatPredicate, IntPredicate,
};
use sql_parse::{BinaryOperator, Expression, Identifier, IdentifierPart, UnaryOperator};
use std::{
    ops::Range,
    sync::atomic::{AtomicU64, Ordering},
};

/// Unique counter for JIT filter function names.
/// Function names must be unique across ALL compiled filters: multiple mappers on the
/// same record otherwise compile a same-named function into separate modules that share
/// one LLVM context, and the second mapper resolves the wrong filter code.
static FILTER_FUNC_SEQ: AtomicU64 = AtomicU64::new(0);

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
    // unique function name per compiled filter (record id alone collides across mappers
    // sharing the same LLVM context)
    let seq = FILTER_FUNC_SEQ.fetch_add(1, Ordering::SeqCst);
    let func_name = &format!("record_filter_{}_{}", record.id(), seq); // Create unique function name
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
        Expression::Unary {
            op,
            op_span: _,
            operand,
        } => parse_unary_exp(context, expr, operand, record, issues, walker, op), // Parse unary expressions like minus/negation
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

/// Parses a unary expression (like negation) by handling different unary operators.
/// Currently only handles the minus operator for negative values.
fn parse_unary_exp<'ctx>(
    context: &GenContext<'ctx>,
    expr: &Expression<'_>,
    sub_expr: &Expression<'_>,
    record: &Record,
    issues: &mut Vec<String>,
    walker: &mut AtomicU64,
    op: &UnaryOperator,
) -> Option<StructValue<'ctx>> {
    match op {
        UnaryOperator::Binary => parse_unsupported(expr, issues),
        UnaryOperator::Collate => parse_unsupported(expr, issues),
        UnaryOperator::LogicalNot => parse_unsupported(expr, issues),
        UnaryOperator::Minus => parse_negative_val(context, sub_expr, record, issues, walker), // Handle negative values
        UnaryOperator::Not => parse_unsupported(expr, issues),
    }
}

/// Parses a negative value by negating the result of the sub-expression.
/// Handles both integer and floating-point negation in the JIT context.
fn parse_negative_val<'ctx>(
    context: &GenContext<'ctx>,
    expr: &Expression<'_>,
    record: &Record,
    issues: &mut Vec<String>,
    walker: &mut AtomicU64,
) -> Option<StructValue<'ctx>> {
    if let Some(ret) = parse_exp(context, expr, record, issues, walker) {
        let ret_type = ret.get_field_at_index(0).unwrap().into_int_value();
        let ret_val_enum = ret.get_field_at_index(1).unwrap();
        let opt_ret_val = match ret_val_enum {
            BasicValueEnum::IntValue(int_value) => Some(
                context
                    .func_generator
                    .builder
                    .build_int_neg(int_value, "int_v")
                    .unwrap()
                    .into(),
            ),
            BasicValueEnum::FloatValue(float_value) => Some(
                context
                    .func_generator
                    .builder
                    .build_float_neg(float_value, "float_v")
                    .unwrap()
                    .into(),
            ),
            _ => None,
        };
        if let Some(ret_val) = opt_ret_val {
            Some(
                context
                    .func_generator
                    .get_ret_val_type()
                    .const_named_struct(&[ret_type.into(), ret_val]),
            )
        } else {
            parse_unsupported(expr, issues)
        }
    } else {
        None
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

/// Generates LLVM code to call a column fetch function based on the column identifier.
/// Returns the fetched value as an LLVM struct with type and value fields.
fn gen_call_fetch_column<'ctx>(
    context: &GenContext<'ctx>, // Generation context with LLVM builder and types
    idp: &IdentifierPart<'_>,   // Identifier part containing the column name
    record: &Record,            // Record definition for column access
    issues: &mut Vec<String>,   // Vector to collect error messages
) -> Option<StructValue<'ctx>> {
    match idp {
        IdentifierPart::Name(id) => gen_call_fetch_column_by_id(context, id, record, issues),
        _ => {
            // Add error for unsupported identifier types
            issues.push(format!("unsupported id: {idp:?}"));
            None
        }
    }
}

fn gen_call_fetch_column_by_id<'ctx>(
    context: &GenContext<'ctx>,
    id: &Identifier<'_>,
    record: &Record,
    issues: &mut Vec<String>,
) -> Option<StructValue<'ctx>> {
    let name = id.as_str();
    // Get the column name as a string; unknown columns must not panic (return an issue instead)
    let column_id = match record.column_id(name) {
        Some(cid) => *cid,
        None => {
            issues.push(format!("column [{name}] is not found in record"));
            return None;
        }
    };
    let column = record.column(column_id);
    // Get the column definition
    let i16_type = context.func_generator.context.i16_type();
    // Get i16 type for IDs

    // Create constants for record ID and column ID
    let param_record_id = i16_type.const_int(record.id() as u64, true);
    let param_column_id = i16_type.const_int(column_id as u64, true);

    let fetch_val_func = choose_fetch_val_fn(context, column);

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
    let val_enum = call_site_value.try_as_basic_value().unwrap_basic();
    // Get the return value
    match val_enum {
        BasicValueEnum::StructValue(struct_value) => {
            conv_rust_type_to_llvm_struct(context, struct_value)
        }
        _ => {
            // Add error if the return value is not a struct
            issues.push(format!("unsupported BasicValueEnum: {val_enum:?}"));
            None
        }
    }
}
