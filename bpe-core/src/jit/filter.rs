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

pub(crate) fn gen_select_filter_func<'ctx>(
    func_generator: &'ctx FuncGenerator<'ctx>,
    where_: &Option<(Expression<'_>, Range<usize>)>,
    record: &Record,
    issues: &mut Vec<String>,
) -> Result<JitFunction<'ctx, FilterFunc>, String> {
    let i64_type = func_generator.context.i64_type();
    let bool_type = func_generator.context.bool_type();
    let fn_type = bool_type.fn_type(&[i64_type.into()], true);
    let func_name = &format!("{}_{}", "record_filter", record.id());
    let func = func_generator.module.add_function(func_name, fn_type, None);
    let block = func_generator.context.append_basic_block(func, "entry");
    func_generator.builder.position_at_end(block);
    let mut walker = AtomicU64::new(0);
    let ret;
    if let Some(where_part) = where_ {
        let param_u64ptr = func.get_nth_param(0).unwrap().into_int_value();
        let i2f_func = func_generator.module.get_function("i2f").unwrap();
        let log_func = func_generator.module.get_function("log").unwrap();
        let context = GenContext::new(func_generator, param_u64ptr, i2f_func, log_func);
        let opt_val = parse_exp(&context, &where_part.0, record, issues, &mut walker);
        if let Some(val) = opt_val {
            let type_val = val.get_field_at_index(0).unwrap().into_int_value();
            let bool_val = i64_type.const_int(T_B64, true);
            let type_check_ret = func_generator
                .builder
                .build_int_compare(IntPredicate::EQ, type_val, bool_val, "type_check_status")
                .unwrap();
            let ret_val = val.get_field_at_index(1).unwrap().into_int_value();
            let true_val = i64_type.const_int(B_TRUE, true);
            let val_check_ret = func_generator
                .builder
                .build_int_compare(IntPredicate::EQ, ret_val, true_val, "val_check_status")
                .unwrap();
            ret = func_generator
                .builder
                .build_and(type_check_ret, val_check_ret, "ret_status")
                .unwrap();
        } else {
            return Err("failed to gen filter function".to_string());
        }
    } else {
        ret = bool_type.const_int(T_B64, true);
    }
    let _ = func_generator.builder.build_return(Some(&ret));
    let opt_filter_func = func_generator.compile::<FilterFunc>(func_name);
    if let Some(filter_func) = opt_filter_func {
        Ok(filter_func)
    } else {
        Err("failed to compile filter function".to_string())
    }
}

fn parse_exp<'ctx>(
    context: &GenContext<'ctx>,
    expr: &Expression<'_>,
    record: &Record,
    issues: &mut Vec<String>,
    walker: &mut AtomicU64,
) -> Option<StructValue<'ctx>> {
    match expr {
        Expression::Binary {
            op,
            op_span: _,
            lhs,
            rhs,
        } => {
            let rs = parse_binary_exp(context, record, issues, walker, lhs, rhs);
            if let Ok(bin_exp) = rs {
                bin_op_calc(context, expr, issues, walker, op, &bin_exp)
            } else {
                issues.push(format!("unsupported expression: {expr:?}, {:?}", rs.err()));
                None
            }
        }
        Expression::Identifier(id_vec) => parse_identifier(context, record, issues, id_vec),
        Expression::Integer(group) => parse_val(context, T_I64, group.0),
        Expression::Float(group) => parse_val(context, T_F64, group.0.to_bits()),
        _ => parse_unsupported(expr, issues),
    }
}

fn parse_binary_exp<'ctx>(
    context: &GenContext<'ctx>,
    record: &Record,
    issues: &mut Vec<String>,
    walker: &mut AtomicU64,
    lhs: &Expression<'_>,
    rhs: &Expression<'_>,
) -> Result<BinaryExpression<'ctx>, String> {
    let opt_l_ret = parse_exp(context, lhs, record, issues, walker);
    let opt_r_ret = parse_exp(context, rhs, record, issues, walker);
    if opt_l_ret.is_some() && opt_r_ret.is_some() {
        let l_ret = unwrap_opt(opt_l_ret);
        let r_ret = unwrap_opt(opt_r_ret);
        let l_val_type = l_ret.get_field_at_index(0).unwrap();
        let r_val_type = r_ret.get_field_at_index(0).unwrap();
        let l_val = l_ret.get_field_at_index(1).unwrap();
        let r_val = r_ret.get_field_at_index(1).unwrap();
        Ok(BinaryExpression::new(l_val_type, r_val_type, l_val, r_val))
    } else if opt_l_ret.is_none() {
        Err("failed to parse left expression".to_string())
    } else {
        Err("failed to parse right expression".to_string())
    }
}

fn unwrap_opt(opt: Option<StructValue<'_>>) -> StructValue<'_> {
    match opt {
        Some(v) => v,
        _ => panic!(),
    }
}

fn parse_identifier<'ctx>(
    context: &GenContext<'ctx>,
    record: &Record,
    issues: &mut Vec<String>,
    id_vec: &Vec<IdentifierPart<'_>>,
) -> Option<StructValue<'ctx>> {
    let len = id_vec.len();
    if len == 1 {
        let idp = id_vec.first().unwrap();
        gen_call_fetch_column(context, idp, record, issues)
    } else if len == 2 {
        // todo table.column, currently just support column
        let _tab_id_part = id_vec.first().unwrap();
        let idp = id_vec.get(1).unwrap();
        gen_call_fetch_column(context, idp, record, issues)
    } else {
        issues.push(format!("unsupported id: {id_vec:?}"));
        None
    }
}

fn parse_val<'ctx>(
    context: &GenContext<'ctx>,
    val_type: u64,
    val: u64,
) -> Option<StructValue<'ctx>> {
    let i64_type = context.func_generator.context.i64_type();
    let data_type = i64_type.const_int(val_type, true);
    let data = i64_type.const_int(val, true);
    let ret_val_type = context.func_generator.get_ret_val_type();
    Some(ret_val_type.const_named_struct(&[data_type.into(), data.into()]))
}

fn parse_unsupported<'ctx>(
    expr: &Expression<'_>,
    issues: &mut Vec<String>,
) -> Option<StructValue<'ctx>> {
    let issue_desc = match expr {
        Expression::String(str) => format!("unsupported String: {str:?}"),
        Expression::Function(f, expr_vec, _) => {
            format!("unsupported func: {f:?}, expr_vec: {expr_vec:?}")
        }
        _ => format!("unsupported expr condition: {expr:?}"),
    };
    issues.push(issue_desc);
    None
}

fn bin_op_calc<'ctx>(
    context: &GenContext<'ctx>,
    expr: &Expression<'_>,
    issues: &mut Vec<String>,
    walker: &mut AtomicU64,
    op: &BinaryOperator,
    bin_exp: &BinaryExpression<'ctx>,
) -> Option<StructValue<'ctx>> {
    match op {
        BinaryOperator::Or => logic_op(context, walker, bin_exp, |builder, v1, v2, walker| {
            let w = walker.fetch_add(1, Ordering::SeqCst);
            builder.build_or(v1, v2, &format!("val_{w}_or")).unwrap()
        }),
        BinaryOperator::And => logic_op(context, walker, bin_exp, |builder, v1, v2, walker| {
            let w = walker.fetch_add(1, Ordering::SeqCst);
            builder.build_and(v1, v2, &format!("val_{w}_and")).unwrap()
        }),
        BinaryOperator::Eq
        | BinaryOperator::GtEq
        | BinaryOperator::Gt
        | BinaryOperator::LtEq
        | BinaryOperator::Lt
        | BinaryOperator::Neq => {
            let (matched, int_op, float_op, name) = match op {
                BinaryOperator::Eq => (true, IntPredicate::EQ, FloatPredicate::OEQ, "eq"),
                BinaryOperator::GtEq => (true, IntPredicate::SGE, FloatPredicate::OGE, "gteq"),
                BinaryOperator::Gt => (true, IntPredicate::SGT, FloatPredicate::OGT, "gt"),
                BinaryOperator::LtEq => (true, IntPredicate::SLE, FloatPredicate::OLE, "lteq"),
                BinaryOperator::Lt => (true, IntPredicate::SLT, FloatPredicate::OLT, "lt"),
                BinaryOperator::Neq => (true, IntPredicate::NE, FloatPredicate::ONE, "neq"),
                _ => (false, IntPredicate::EQ, FloatPredicate::OEQ, "!!!op"),
            };
            if matched {
                logic_compare(context, walker, bin_exp, int_op, float_op, name)
            } else {
                issues.push(format!("unsupported op: {op:?}"));
                None
            }
        }
        _ => {
            issues.push(format!("unsupported expr condition: {expr:?}"));
            None
        }
    }
}

fn logic_op<'ctx>(
    context: &GenContext<'ctx>,
    walker: &mut AtomicU64,
    bin_exp: &BinaryExpression<'ctx>,
    f: LogicOpFnType<'ctx>,
) -> Option<StructValue<'ctx>> {
    let i64_type = context.func_generator.context.i64_type();
    let l_val = bin_exp.l_val.into_int_value();
    let r_val = bin_exp.r_val.into_int_value();
    let ret_type = i64_type.const_int(T_B64, true);
    let ret_val = f(&context.func_generator.builder, l_val, r_val, walker);
    Some(
        context
            .func_generator
            .get_ret_val_type()
            .const_named_struct(&[ret_type.into(), ret_val.into()]),
    )
}

fn logic_compare<'ctx>(
    context: &GenContext<'ctx>,
    walker: &mut AtomicU64,
    bin_exp: &BinaryExpression<'ctx>,
    int_op: IntPredicate,
    float_op: FloatPredicate,
    name: &str,
) -> Option<StructValue<'ctx>> {
    let w = walker.fetch_add(1, Ordering::SeqCst);
    let func_name = &format!("val_{w}_{name}");
    let i64_type = context.func_generator.context.i64_type();
    let l_val_type = bin_exp.l_val_type.into_int_value();
    let r_val_type = bin_exp.r_val_type.into_int_value();

    let t_f64 = i64_type.const_int(T_F64, true);
    let is_float = check_is_float_cmp(context, l_val_type, r_val_type, t_f64);

    // Get the current function to append basic blocks to
    let current_block = context.func_generator.builder.get_insert_block().unwrap();
    let current_func = current_block.get_parent().unwrap();

    // Create basic blocks for different comparison paths
    let float_cmp_block = context
        .func_generator
        .context
        .append_basic_block(current_func, &format!("{}_float_cmp", func_name));
    let int_cmp_block = context
        .func_generator
        .context
        .append_basic_block(current_func, &format!("{}_int_cmp", func_name));
    let merge_block = context
        .func_generator
        .context
        .append_basic_block(current_func, &format!("{}_merge", func_name));

    // Branch to appropriate comparison block
    context
        .func_generator
        .builder
        .build_conditional_branch(is_float, float_cmp_block, int_cmp_block)
        .unwrap();

    let float_result = build_float_cmp(
        context,
        float_op,
        i64_type,
        bin_exp,
        float_cmp_block,
        merge_block,
    );

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

    let ret_type = i64_type.const_int(T_B64, true);
    let ret_val = ret_phi_int_val(
        context,
        func_name,
        i64_type,
        float_cmp_block,
        int_cmp_block,
        float_result,
        int_result,
    );
    Some(
        context
            .func_generator
            .get_ret_val_type()
            .const_named_struct(&[ret_type.into(), ret_val.into()]),
    )
}

fn check_is_float_cmp<'ctx>(
    context: &GenContext<'ctx>,
    l_val_type: IntValue<'ctx>,
    r_val_type: IntValue<'ctx>,
    t_f64: IntValue<'ctx>,
) -> IntValue<'ctx> {
    let l_float = context
        .func_generator
        .builder
        .build_int_compare(IntPredicate::EQ, l_val_type, t_f64, "is_l_float")
        .unwrap();
    let r_float = context
        .func_generator
        .builder
        .build_int_compare(IntPredicate::EQ, r_val_type, t_f64, "is_r_float")
        .unwrap();
    context
        .func_generator
        .builder
        .build_or(l_float, r_float, "is_float")
        .unwrap()
}

fn build_float_cmp<'ctx>(
    context: &GenContext<'ctx>,
    _float_op: FloatPredicate,
    i64_type: IntType<'ctx>,
    bin_exp: &BinaryExpression<'ctx>,
    float_cmp_block: BasicBlock<'ctx>,
    merge_block: BasicBlock<'ctx>,
) -> IntValue<'ctx> {
    // Float comparison block
    context
        .func_generator
        .builder
        .position_at_end(float_cmp_block);
    let l_val_type = bin_exp.l_val_type.into_int_value();
    let r_val_type = bin_exp.r_val_type.into_int_value();
    let l_val = bin_exp.l_val.into_int_value();
    let r_val = bin_exp.r_val.into_int_value();
    // float comp
    let _l_float_val = convert_int2float(context, "l", l_val_type, l_val);
    let _r_float_val = convert_int2float(context, "r", r_val_type, r_val);
    let float_result_int = i64_type.const_int(T_B64, true);
    let float_result = context
        .func_generator
        .builder
        .build_int_cast(float_result_int, i64_type, "float_result")
        .unwrap();
    context
        .func_generator
        .builder
        .build_unconditional_branch(merge_block)
        .unwrap();
    float_result
}

fn convert_int2float<'ctx>(
    context: &GenContext<'ctx>,
    flag: &str,
    val_type: IntValue<'ctx>,
    val: IntValue<'ctx>,
) -> FloatValue<'ctx> {
    context
        .func_generator
        .builder
        .build_call(
            context.i2f_func,
            &[val_type.into(), val.into()],
            &format!("{flag}_i2f"),
        )
        .unwrap()
        .try_as_basic_value()
        .unwrap_basic()
        .into_float_value()
}

fn build_int_cmp<'ctx>(
    context: &GenContext<'ctx>,
    int_op: IntPredicate,
    i64_type: IntType<'ctx>,
    bin_exp: &BinaryExpression<'ctx>,
    int_cmp_block: BasicBlock<'ctx>,
    merge_block: BasicBlock<'ctx>,
) -> IntValue<'ctx> {
    // Int comparison block
    context
        .func_generator
        .builder
        .position_at_end(int_cmp_block);
    let l_val = bin_exp.l_val.into_int_value();
    let r_val = bin_exp.r_val.into_int_value();
    // int comp
    let int_result_int = context
        .func_generator
        .builder
        .build_int_compare(int_op, l_val, r_val, "int_cmp")
        .unwrap();
    let int_result = context
        .func_generator
        .builder
        .build_int_cast(int_result_int, i64_type, "int_result")
        .unwrap();
    context
        .func_generator
        .builder
        .build_unconditional_branch(merge_block)
        .unwrap();
    int_result
}

fn ret_phi_int_val<'ctx>(
    context: &GenContext<'ctx>,
    func_name: &String,
    i64_type: IntType<'ctx>,
    block1: BasicBlock<'ctx>,
    block2: BasicBlock<'ctx>,
    result1: IntValue<'ctx>,
    result2: IntValue<'ctx>,
) -> IntValue<'ctx> {
    // Create PHI node to merge results from both paths
    let phi = context
        .func_generator
        .builder
        .build_phi(i64_type, &format!("{}_phi", func_name))
        .unwrap();
    phi.add_incoming(&[(&result1, block1), (&result2, block2)]);
    phi.as_basic_value().into_int_value()
}

fn gen_call_fetch_column<'ctx>(
    context: &GenContext<'ctx>,
    idp: &IdentifierPart<'_>,
    record: &Record,
    issues: &mut Vec<String>,
) -> Option<StructValue<'ctx>> {
    match idp {
        IdentifierPart::Name(id) => {
            let name = id.as_str();
            let column_id = *record.column_id(name).unwrap();
            let column = record.column(column_id);
            let i16_type = context.func_generator.context.i16_type();
            let param_record_id = i16_type.const_int(record.id() as u64, true);
            let param_column_id = i16_type.const_int(column_id as u64, true);
            let fetch_val_func = match column.data_type() {
                ColumnType::Long => context.func_generator.module.get_function("fetch_i64"),
                ColumnType::Double => context.func_generator.module.get_function("fetch_f64"),
            }
            .unwrap();
            let call_site_value = context
                .func_generator
                .builder
                .build_call(
                    fetch_val_func,
                    &[
                        context.param_u64ptr.into(),
                        param_record_id.into(),
                        param_column_id.into(),
                    ],
                    "ret",
                )
                .unwrap();
            let val_enum = call_site_value.try_as_basic_value().unwrap_basic();
            match val_enum {
                BasicValueEnum::StructValue(struct_value) => {
                    // 提取字段
                    let data_type = context
                        .func_generator
                        .builder
                        .build_extract_value(struct_value, 0, "data_type")
                        .unwrap()
                        .into_int_value();
                    let data = context
                        .func_generator
                        .builder
                        .build_extract_value(struct_value, 1, "data")
                        .unwrap()
                        .into_int_value();
                    let ret_val_type = context.func_generator.get_ret_val_type();
                    Some(ret_val_type.const_named_struct(&[data_type.into(), data.into()]))
                }
                _ => {
                    issues.push(format!("unsupported BasicValueEnum: {val_enum:?}"));
                    None
                }
            }
        }
        _ => {
            issues.push(format!("unsupported id: {idp:?}"));
            None
        }
    }
}
