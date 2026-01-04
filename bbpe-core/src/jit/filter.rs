use crate::{
    data::{ColumnType, Record},
    jit::base::{FuncGenerator, T_ERR, T_F64, T_I64},
    sql::base::FilterFunc,
};
use inkwell::{
    builder::Builder,
    execution_engine::JitFunction,
    values::{BasicValueEnum, FunctionValue, IntValue, StructValue},
    FloatPredicate, IntPredicate,
};
use sql_parse::{BinaryOperator, Expression, IdentifierPart};
use std::ops::Range;

type LogicOpFnType<'ctx> =
    fn(&Builder<'ctx>, IntValue<'ctx>, IntValue<'ctx>, usize) -> IntValue<'ctx>;

#[derive(Debug)]
struct BinaryExpression<'a> {
    lhs_val_type: BasicValueEnum<'a>,
    rhs_val_type: BasicValueEnum<'a>,
    lhs_ret_val: BasicValueEnum<'a>,
    rhs_ret_val: BasicValueEnum<'a>,
}
impl<'a> BinaryExpression<'a> {
    fn new(
        lhs_val_type: BasicValueEnum<'a>,
        rhs_val_type: BasicValueEnum<'a>,
        lhs_ret_val: BasicValueEnum<'a>,
        rhs_ret_val: BasicValueEnum<'a>,
    ) -> Self {
        Self {
            lhs_val_type,
            rhs_val_type,
            lhs_ret_val,
            rhs_ret_val,
        }
    }
}

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
    log::info!("try to genterator func, named: {func_name}");
    let func = func_generator.module.add_function(func_name, fn_type, None);
    let block = func_generator.context.append_basic_block(func, "entry");
    func_generator.builder.position_at_end(block);
    let ret;
    if let Some(where_part) = where_ {
        let param_u64ptr = func.get_nth_param(0).unwrap().into_int_value();
        let opt_val = gen_filter_func(
            func_generator,
            &func,
            &param_u64ptr,
            &where_part.0,
            record,
            issues,
            0,
        );
        if let Some(val) = opt_val {
            let ret_val = val.get_field_at_index(0).unwrap().into_int_value();
            let err_val = i64_type.const_int(T_ERR as u64, true);
            ret = func_generator
                .builder
                .build_int_compare(IntPredicate::EQ, ret_val, err_val, "ret_status")
                .unwrap();
            // log::info!("filter func ret: {val}/{ret}");
        } else {
            return Err("failed to gen filter function".to_string());
        }
    } else {
        ret = bool_type.const_int(1, true);
    }
    let _ = func_generator.builder.build_return(Some(&ret));
    let opt_filter_func = func_generator.compile::<FilterFunc>(func_name);
    if let Some(filter_func) = opt_filter_func {
        Ok(filter_func)
    } else {
        Err("failed to compile filter function".to_string())
    }
}

fn gen_filter_func<'ctx>(
    func_generator: &'ctx FuncGenerator<'ctx>,
    _func: &FunctionValue<'ctx>,
    param_u64ptr: &IntValue<'ctx>,
    expr: &Expression<'_>,
    record: &Record,
    issues: &mut Vec<String>,
    walker: usize,
) -> Option<StructValue<'ctx>> {
    match expr {
        Expression::Binary {
            op,
            op_span: _,
            lhs,
            rhs,
        } => {
            let lhs_val = gen_filter_func(
                func_generator,
                _func,
                param_u64ptr,
                lhs,
                record,
                issues,
                walker + 1,
            )
            .unwrap();
            let rhs_val = gen_filter_func(
                func_generator,
                _func,
                param_u64ptr,
                rhs,
                record,
                issues,
                walker + 2,
            )
            .unwrap();
            let lhs_val_type = lhs_val.get_field_at_index(0).unwrap();
            let rhs_val_type = rhs_val.get_field_at_index(0).unwrap();
            let lhs_ret_val = lhs_val.get_field_at_index(1).unwrap();
            let rhs_ret_val = rhs_val.get_field_at_index(1).unwrap();
            let bin_exp =
                BinaryExpression::new(lhs_val_type, rhs_val_type, lhs_ret_val, rhs_ret_val);
            bin_op_calc(func_generator, expr, issues, walker, op, &bin_exp)
        }
        Expression::Identifier(id_vec) => {
            let len = id_vec.len();
            if len == 1 {
                let idp = id_vec.first().unwrap();
                gen_call_fetch_column(func_generator, param_u64ptr, idp, record, issues)
            } else if len == 2 {
                // todo table.column, currently just support column
                let _tab_id_part = id_vec.first().unwrap();
                let idp = id_vec.get(1).unwrap();
                // log::info!("Identifier Vec: {_tab_id_part:?}/{idp:?}");
                gen_call_fetch_column(func_generator, param_u64ptr, idp, record, issues)
            } else {
                issues.push(format!("unsupported id: {id_vec:?}"));
                None
            }
        }
        Expression::Integer(group) => {
            let i8_type = func_generator.context.i8_type();
            let i64_type = func_generator.context.i64_type();
            let data_type = i8_type.const_int(T_I64 as u64, true);
            let data = i64_type.const_int(group.0, true);
            let ret_val_type = func_generator.get_ret_val_type();
            Some(ret_val_type.const_named_struct(&[data_type.into(), data.into()]))
        }
        Expression::Float(group) => {
            let i8_type = func_generator.context.i8_type();
            let i64_type = func_generator.context.i64_type();
            let data_type = i8_type.const_int(T_I64 as u64, true);
            let data = i64_type.const_int(group.0.to_bits(), true);
            let ret_val_type = func_generator.get_ret_val_type();
            Some(ret_val_type.const_named_struct(&[data_type.into(), data.into()]))
        }
        Expression::String(_str) => {
            issues.push(format!("unsupported String: {_str:?}"));
            None
        }
        Expression::Function(f, expr_vec, _) => {
            issues.push(format!("unsupported func: {f:?}, expr_vec: {expr_vec:?}"));
            None
        }
        _ => {
            issues.push(format!("unsupported expr condition: {expr:?}"));
            None
        }
    }
}

fn bin_op_calc<'ctx>(
    func_generator: &'ctx FuncGenerator<'ctx>,
    expr: &Expression<'_>,
    issues: &mut Vec<String>,
    walker: usize,
    op: &BinaryOperator,
    bin_exp: &BinaryExpression<'ctx>,
) -> Option<StructValue<'ctx>> {
    match op {
        BinaryOperator::Or => logic_op(
            func_generator,
            walker,
            bin_exp,
            |builder, v1, v2, walker| {
                builder
                    .build_or(v1, v2, &format!("val_{walker}_or"))
                    .unwrap()
            },
        ),
        BinaryOperator::And => logic_op(
            func_generator,
            walker,
            bin_exp,
            |builder, v1, v2, walker| {
                builder
                    .build_and(v1, v2, &format!("val_{walker}_and"))
                    .unwrap()
            },
        ),
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
                logic_compare(func_generator, walker, bin_exp, int_op, float_op, name)
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
    func_generator: &'ctx FuncGenerator<'ctx>,
    walker: usize,
    bin_exp: &BinaryExpression<'ctx>,
    f: LogicOpFnType<'ctx>,
) -> Option<StructValue<'ctx>> {
    let (i64_type, lhs_val_type, rhs_val_type, lhs_ret_val, rhs_ret_val) = (
        func_generator.context.i64_type(),
        bin_exp.lhs_val_type,
        bin_exp.rhs_val_type,
        bin_exp.lhs_ret_val,
        bin_exp.rhs_ret_val,
    );
    let (ret_type, ret_val) = match (lhs_val_type, rhs_val_type) {
        (BasicValueEnum::IntValue(_), BasicValueEnum::IntValue(_)) => (
            i64_type.const_int(T_I64 as u64, true),
            f(
                &func_generator.builder,
                lhs_ret_val.into_int_value(),
                rhs_ret_val.into_int_value(),
                walker,
            ),
        ),
        _ => panic!("not support"),
    };
    Some(
        func_generator
            .get_ret_val_type()
            .const_named_struct(&[ret_type.into(), ret_val.into()]),
    )
}

fn logic_compare<'ctx>(
    func_generator: &'ctx FuncGenerator<'ctx>,
    walker: usize,
    bin_exp: &BinaryExpression<'ctx>,
    int_op: IntPredicate,
    float_op: FloatPredicate,
    name: &str,
) -> Option<StructValue<'ctx>> {
    let (i64_type, lhs_val_type, rhs_val_type, lhs_ret_val, rhs_ret_val) = (
        func_generator.context.i64_type(),
        bin_exp.lhs_val_type,
        bin_exp.rhs_val_type,
        bin_exp.lhs_ret_val,
        bin_exp.rhs_ret_val,
    );
    let func_name = &format!("val_{walker}_{name}");
    // log::info!("{func_name}, lhs_val_type: {lhs_val_type:?}, rhs_val_type: {rhs_val_type:?}");
    match (lhs_val_type, rhs_val_type) {
        (BasicValueEnum::IntValue(_), BasicValueEnum::IntValue(_)) => {
            // log::info!("{func_name}, int_op: {int_op:?}");
            let ret_type = i64_type.const_int(T_I64 as u64, true);
            let ret_val = func_generator
                .builder
                .build_int_compare(
                    int_op,
                    lhs_ret_val.into_int_value(),
                    rhs_ret_val.into_int_value(),
                    func_name,
                )
                .unwrap();
            Some(
                func_generator
                    .get_ret_val_type()
                    .const_named_struct(&[ret_type.into(), ret_val.into()]),
            )
        }
        (BasicValueEnum::IntValue(_), BasicValueEnum::FloatValue(_))
        | (BasicValueEnum::FloatValue(_), BasicValueEnum::IntValue(_))
        | (BasicValueEnum::FloatValue(_), BasicValueEnum::FloatValue(_)) => {
            // log::info!("{func_name}, int_op: {float_op:?}");
            let ret_type = i64_type.const_int(T_F64 as u64, true);
            let ret_val = func_generator
                .builder
                .build_float_compare(
                    float_op,
                    lhs_ret_val.into_float_value(),
                    rhs_ret_val.into_float_value(),
                    func_name,
                )
                .unwrap();
            Some(
                func_generator
                    .get_ret_val_type()
                    .const_named_struct(&[ret_type.into(), ret_val.into()]),
            )
        }
        _ => panic!("not support"),
    }
}

fn gen_call_fetch_column<'ctx>(
    func_generator: &FuncGenerator<'ctx>,
    param_u64ptr: &IntValue<'ctx>,
    idp: &IdentifierPart<'_>,
    record: &Record,
    issues: &mut Vec<String>,
) -> Option<StructValue<'ctx>> {
    match idp {
        IdentifierPart::Name(id) => {
            let name = id.as_str();
            let column_id = *record.column_id(name).unwrap();
            let column = record.column(column_id);
            let i16_type = func_generator.context.i16_type();
            let param_record_id = i16_type.const_int(record.id() as u64, true);
            let param_column_id = i16_type.const_int(column_id as u64, true);
            let fetch_val_func = match column.data_type() {
                ColumnType::Long => func_generator.module.get_function("fetch_i64"),
                ColumnType::Double => func_generator.module.get_function("fetch_f64"),
            }
            .unwrap();
            let call_site_value = func_generator
                .builder
                .build_call(
                    fetch_val_func,
                    &[
                        (*param_u64ptr).into(),
                        param_record_id.into(),
                        param_column_id.into(),
                    ],
                    "ret",
                )
                .unwrap();
            let val_enum = call_site_value.try_as_basic_value().unwrap_basic();
            match val_enum {
                BasicValueEnum::StructValue(struct_value) => {
                    // log::info!("fetch_column: {name}, {struct_value:?}");
                    Some(struct_value)
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
