use crate::{
    data::{ColumnType, Record},
    jit::base::FuncGenerator,
    sql::base::FilterFunc,
};
use inkwell::{execution_engine::JitFunction, values::IntValue, IntPredicate};
use sql_parse::{BinaryOperator, Expression, IdentifierPart};
use std::ops::Range;

pub(crate) fn gen_select_filter_func<'ctx>(
    func_generator: &'ctx FuncGenerator<'ctx>,
    where_: &Option<(Expression<'_>, Range<usize>)>,
    record: &Record,
    issues: &mut Vec<String>,
) -> Result<JitFunction<'ctx, FilterFunc>, String> {
    let i64_type = func_generator.context.i64_type();
    let bool_type = func_generator.context.bool_type();
    let fn_type = bool_type.fn_type(&[i64_type.into()], false);
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
            &param_u64ptr,
            &where_part.0,
            record,
            issues,
            0,
        );
        if let Some(val) = opt_val {
            ret = val;
        } else {
            return Err("failed to gen filter function".to_string());
        }
    } else {
        ret = bool_type.const_int(1, false);
    }
    let _ = func_generator.builder.build_return(Some(&ret));
    let opt_filter_func = func_generator.compile::<FilterFunc>(func_name);
    if let Some(filter_func) = opt_filter_func {
        log::info!("filter func: {filter_func:#?}");
        Ok(filter_func)
    } else {
        Err("failed to compile filter function".to_string())
    }
}

fn gen_filter_func<'ctx>(
    func_generator: &FuncGenerator<'ctx>,
    param_u64ptr: &IntValue<'ctx>,
    expr: &Expression<'_>,
    record: &Record,
    issues: &mut Vec<String>,
    walker: usize,
) -> Option<IntValue<'ctx>> {
    match expr {
        Expression::Binary {
            op,
            op_span: _,
            lhs,
            rhs,
        } => match op {
            BinaryOperator::Or => {
                let lhs_val = gen_filter_func(
                    func_generator,
                    param_u64ptr,
                    lhs,
                    record,
                    issues,
                    walker + 1,
                )
                .unwrap();
                let rhs_val = gen_filter_func(
                    func_generator,
                    param_u64ptr,
                    rhs,
                    record,
                    issues,
                    walker + 2,
                )
                .unwrap();
                let val = func_generator
                    .builder
                    .build_or(lhs_val, rhs_val, &format!("val_{walker}_or"))
                    .unwrap();
                Some(val)
            }
            BinaryOperator::And => {
                let lhs_val = gen_filter_func(
                    func_generator,
                    param_u64ptr,
                    lhs,
                    record,
                    issues,
                    walker + 1,
                )
                .unwrap();
                let rhs_val = gen_filter_func(
                    func_generator,
                    param_u64ptr,
                    rhs,
                    record,
                    issues,
                    walker + 2,
                )
                .unwrap();
                let val = func_generator
                    .builder
                    .build_and(lhs_val, rhs_val, &format!("val_{walker}_and"))
                    .unwrap();
                Some(val)
            }
            BinaryOperator::Eq => {
                let lhs_val = gen_filter_func(
                    func_generator,
                    param_u64ptr,
                    lhs,
                    record,
                    issues,
                    walker + 1,
                )
                .unwrap();
                let rhs_val = gen_filter_func(
                    func_generator,
                    param_u64ptr,
                    rhs,
                    record,
                    issues,
                    walker + 2,
                )
                .unwrap();
                let val = func_generator
                    .builder
                    .build_int_compare(
                        IntPredicate::EQ,
                        lhs_val,
                        rhs_val,
                        &format!("val_{walker}_eq"),
                    )
                    .unwrap();
                Some(val)
            }
            BinaryOperator::GtEq => {
                let lhs_val = gen_filter_func(
                    func_generator,
                    param_u64ptr,
                    lhs,
                    record,
                    issues,
                    walker + 1,
                )
                .unwrap();
                let rhs_val = gen_filter_func(
                    func_generator,
                    param_u64ptr,
                    rhs,
                    record,
                    issues,
                    walker + 2,
                )
                .unwrap();
                let val = func_generator
                    .builder
                    .build_int_compare(
                        IntPredicate::SGE,
                        lhs_val,
                        rhs_val,
                        &format!("val_{walker}_gteq"),
                    )
                    .unwrap();
                Some(val)
            }
            BinaryOperator::Gt => {
                let lhs_val = gen_filter_func(
                    func_generator,
                    param_u64ptr,
                    lhs,
                    record,
                    issues,
                    walker + 1,
                )
                .unwrap();
                let rhs_val = gen_filter_func(
                    func_generator,
                    param_u64ptr,
                    rhs,
                    record,
                    issues,
                    walker + 2,
                )
                .unwrap();
                let val = func_generator
                    .builder
                    .build_int_compare(
                        IntPredicate::SGT,
                        lhs_val,
                        rhs_val,
                        &format!("val_{walker}_gt"),
                    )
                    .unwrap();
                Some(val)
            }
            BinaryOperator::LtEq => {
                let lhs_val = gen_filter_func(
                    func_generator,
                    param_u64ptr,
                    lhs,
                    record,
                    issues,
                    walker + 1,
                )
                .unwrap();
                let rhs_val = gen_filter_func(
                    func_generator,
                    param_u64ptr,
                    rhs,
                    record,
                    issues,
                    walker + 2,
                )
                .unwrap();
                let val = func_generator
                    .builder
                    .build_int_compare(
                        IntPredicate::SLE,
                        lhs_val,
                        rhs_val,
                        &format!("val_{walker}_lteq"),
                    )
                    .unwrap();
                Some(val)
            }
            BinaryOperator::Lt => {
                let lhs_val = gen_filter_func(
                    func_generator,
                    param_u64ptr,
                    lhs,
                    record,
                    issues,
                    walker + 1,
                )
                .unwrap();
                let rhs_val = gen_filter_func(
                    func_generator,
                    param_u64ptr,
                    rhs,
                    record,
                    issues,
                    walker + 2,
                )
                .unwrap();
                let val = func_generator
                    .builder
                    .build_int_compare(
                        IntPredicate::SLT,
                        lhs_val,
                        rhs_val,
                        &format!("val_{walker}_lt"),
                    )
                    .unwrap();
                Some(val)
            }
            BinaryOperator::Neq => {
                let lhs_val = gen_filter_func(
                    func_generator,
                    param_u64ptr,
                    lhs,
                    record,
                    issues,
                    walker + 1,
                )
                .unwrap();
                let rhs_val = gen_filter_func(
                    func_generator,
                    param_u64ptr,
                    rhs,
                    record,
                    issues,
                    walker + 2,
                )
                .unwrap();
                let val = func_generator
                    .builder
                    .build_int_compare(
                        IntPredicate::NE,
                        lhs_val,
                        rhs_val,
                        &format!("val_{walker}_neq"),
                    )
                    .unwrap();
                Some(val)
            }
            _ => {
                issues.push(format!("unsupported expr condition: {expr:?}"));
                None
            }
        },
        Expression::Identifier(id_vec) => {
            let len = id_vec.len();
            if len == 1 {
                let idp = id_vec.first().unwrap();
                gen_call_fetch_column(func_generator, param_u64ptr, idp, record, issues)
            } else if len == 2 {
                let idp = id_vec.get(1).unwrap();
                gen_call_fetch_column(func_generator, param_u64ptr, idp, record, issues)
            } else {
                issues.push(format!("unsupported id: {id_vec:?}"));
                None
            }
        }
        Expression::Integer(group) => {
            let i64_type = func_generator.context.i64_type();
            Some(i64_type.const_int(group.0, true))
        }
        Expression::Float(group) => {
            let i64_type = func_generator.context.i64_type();
            // TODO: support float
            Some(i64_type.const_int(group.0 as u64, true))
        }
        Expression::String(_str) => {
            issues.push(format!("unsupported String: {_str:?}"));
            None
        }
        _ => {
            issues.push(format!("unsupported expr condition: {expr:?}"));
            None
        }
    }
}

fn gen_call_fetch_column<'ctx>(
    func_generator: &FuncGenerator<'ctx>,
    param_u64ptr: &IntValue<'ctx>,
    idp: &IdentifierPart<'_>,
    record: &Record,
    issues: &mut Vec<String>,
) -> Option<IntValue<'ctx>> {
    match idp {
        IdentifierPart::Name(id) => {
            let name = id.as_str();
            let column_id = *record.column_id(name).unwrap();
            let column = record.column(column_id);
            let i16_type = func_generator.context.i16_type();
            let param_record_id = i16_type.const_int(record.id() as u64, false);
            let param_column_id = i16_type.const_int(column_id as u64, false);
            let fetch_val_func = match column.data_type() {
                ColumnType::Long => func_generator.module.get_function("fetch_i64"),
                // TODO: support float
                ColumnType::Double => func_generator.module.get_function("fetch_i64"),
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
            let ret = call_site_value
                .try_as_basic_value()
                .left()
                .unwrap()
                .into_int_value();
            Some(ret)
        }
        _ => {
            issues.push(format!("unsupported id: {idp:?}"));
            None
        }
    }
}
