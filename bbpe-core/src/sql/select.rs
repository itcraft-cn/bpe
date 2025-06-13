use crate::{
    consts::DEFAULT_SELECT_SIZE,
    data::Record,
    jit::{base::FuncGenerator, filter::gen_select_filter_func},
    sql::base::{parse_sql, ExprEntity, FilterFunc, ParsedSql, ValType},
};
use inkwell::execution_engine::JitFunction;
use sql_parse::{
    Expression, IdentifierPart, ParseOptions, Select, SelectExpr, Statement, TableReference,
    UnaryOperator,
};
use std::ops::Range;

pub(crate) fn parse_select(sql: &str, options: &ParseOptions) -> Option<ParsedSql> {
    parse_sql(sql, options, |statement| match statement {
        Statement::Select(select_stat) => parse_select_statement(select_stat),
        _ => None,
    })
}

fn parse_select_statement(select_stat: Select<'_>) -> Option<ParsedSql> {
    let mut issues = Vec::new();
    check_forbidden_statement(&select_stat, &mut issues);
    if issues.is_empty() {
        let boxed = Box::new(FuncGenerator::new());
        let func_generator = Box::leak(boxed);
        // 识别表名
        let records = parse_select_target(&select_stat.table_references, &mut issues);
        let record = fetch_record(&records)?;
        let record_id = record.id();
        // 辨识过滤条件 jit
        let opt_filter = parse_select_filter(func_generator, &select_stat, record, &mut issues);
        // 限制数据范围
        let limit_range = parse_select_limitor(&select_stat.limit, &mut issues);
        // 辨识字段
        let fields = parse_select_fields(
            func_generator,
            &select_stat.select_exprs,
            record_id,
            &mut issues,
        );
        if issues.is_empty() {
            if let Some(filter) = opt_filter {
                Some(ParsedSql::new(records, filter, limit_range, fields))
            } else {
                log::warn!("hit failed to parse select filter");
                None
            }
        } else {
            for issue in issues {
                log::warn!("hit issue: [{issue}]");
            }
            None
        }
    } else {
        None
    }
}

fn fetch_record(records: &Vec<u16>) -> Option<&Record> {
    if records.is_empty() {
        log::warn!("no records found in sql query");
        None
    } else if records.len() != 1 {
        log::warn!("not support multi record select, skipping");
        for record in records {
            log::warn!("record id in sql:{record}");
        }
        None
    } else {
        let id = records.first().unwrap();
        Record::get_record(*id)
    }
}

fn check_forbidden_statement(select_stat: &Select<'_>, issues: &mut Vec<String>) {
    if !select_stat.flags.is_empty() {
        issues.push(String::from("select flag is not supported"));
    } else if select_stat.locking.is_some() {
        issues.push(String::from("locking statement is not supported"));
    } else if select_stat.having.is_some() {
        issues.push(String::from("having statement is not supported"));
    } else if select_stat.group_by.is_some() {
        issues.push(String::from("group by statement is not supported"));
    } else if select_stat.order_by.is_some() {
        issues.push(String::from("order by statement is not supported"));
    }
}

fn parse_select_target(
    record_references: &Option<Vec<TableReference<'_>>>,
    issues: &mut Vec<String>,
) -> Vec<u16> {
    if let Some(tab_vec) = record_references {
        let mut tab_ref_vec = vec![];
        for tab in tab_vec {
            if let Some(value) = parse_tab_ref(tab, &mut tab_ref_vec) {
                issues.push(value);
                return vec![];
            }
        }
        tab_ref_vec
    } else {
        issues.push(String::from("need a record name"));
        vec![]
    }
}

fn parse_tab_ref(tab: &TableReference<'_>, tab_ref_vec: &mut Vec<u16>) -> Option<String> {
    match tab {
        TableReference::Table {
            identifier,
            as_span: _,
            as_,
            index_hints: _,
        } => {
            if as_.is_some() {
                return Some(String::from("as is not supported"));
            }
            let name = identifier.identifier.value;
            if let Some(record_id) = Record::fetch_record_id(name) {
                tab_ref_vec.push(record_id);
            } else {
                return Some(format!("record define: [{name}] is not found"));
            }
        }
        TableReference::Query {
            query: _,
            as_span: _,
            as_: _,
        } => {
            return Some(String::from("ref query is not supported"));
        }
        TableReference::Join {
            join: _,
            left: _,
            right: _,
            specification: _,
        } => {
            return Some(String::from("join is not supported"));
        }
    }
    None
}

fn parse_select_filter<'ctx>(
    func_generator: &'ctx FuncGenerator<'ctx>,
    select_stat: &Select<'_>,
    record: &Record,
    issues: &mut Vec<String>,
) -> Option<JitFunction<'ctx, FilterFunc>> {
    let rs_filters_func =
        gen_select_filter_func(func_generator, &select_stat.where_, record, issues);
    if let Ok(filters_func) = rs_filters_func {
        Some(filters_func)
    } else {
        log::warn!("{}", rs_filters_func.err().unwrap());
        for issue in &*issues {
            log::warn!("issue: {issue:#?}");
        }
        None
    }
}

fn parse_select_limitor(
    limit: &Option<(Range<usize>, Option<Expression<'_>>, Expression<'_>)>,
    issues: &mut Vec<String>,
) -> isize {
    if let Some(limit_part) = limit {
        if limit_part.1.is_some() {
            issues.push(String::from("offset is not supported"));
            return 0;
        }
        match &limit_part.2 {
            Expression::Integer(v) => v.0 as isize,
            Expression::Unary {
                op: UnaryOperator::Minus,
                operand,
                ..
            } => match operand.as_ref() {
                Expression::Integer(v) => 0 - v.0 as isize,
                _ => 0 - DEFAULT_SELECT_SIZE,
            },
            _ => DEFAULT_SELECT_SIZE,
        }
    } else {
        DEFAULT_SELECT_SIZE
    }
}

fn parse_select_fields<'ctx>(
    func_generator: &'ctx FuncGenerator<'ctx>,
    select_exprs: &[SelectExpr<'_>],
    record_id: u16,
    issues: &mut Vec<String>,
) -> Vec<ExprEntity> {
    let mut expr_vec = vec![];
    for select_expr in select_exprs {
        if select_expr.as_.is_some() {
            issues.push(String::from("as is not supported"));
            return vec![];
        }
        if let Some(value) = check_not_allow_expr(&select_expr.expr) {
            issues.push(value);
            return vec![];
        }
        if let Some(value) = parse_expr(func_generator, &select_expr.expr, record_id, &mut expr_vec)
        {
            issues.push(value);
            return vec![];
        }
    }
    expr_vec
}

fn check_not_allow_expr(expr: &Expression<'_>) -> Option<String> {
    let not_allow = matches!(
        expr,
        Expression::Subquery(_)
            | Expression::Null(_)
            | Expression::ListHack(_)
            | Expression::WindowFunction {
                function: _,
                args: _,
                function_span: _,
                over_span: _,
                window_spec: _,
            }
            | Expression::Arg(_)
            | Expression::Exists(_)
            | Expression::In {
                lhs: _,
                rhs: _,
                in_span: _,
                not_in: _,
            }
            | Expression::Is(_, _, _)
            | Expression::Invalid(_)
            | Expression::Case {
                case_span: _,
                value: _,
                whens: _,
                else_: _,
                end_span: _,
            }
            | Expression::Cast {
                cast_span: _,
                expr: _,
                as_span: _,
                type_: _,
            }
            | Expression::Count {
                count_span: _,
                distinct_span: _,
                expr: _,
            }
            | Expression::GroupConcat {
                group_concat_span: _,
                distinct_span: _,
                expr: _,
            }
            | Expression::Variable {
                global: _,
                session: _,
                dot: _,
                variable: _,
                variable_span: _,
            }
            | Expression::Unary {
                op: _,
                op_span: _,
                operand: _,
            }
    );
    if not_allow {
        Some(format!("expression {:?} is not supported", &expr))
    } else {
        None
    }
}

fn parse_expr<'ctx>(
    _func_generator: &'ctx FuncGenerator<'ctx>,
    expr: &Expression<'_>,
    record_id: u16,
    expr_entity_vec: &mut Vec<ExprEntity>,
) -> Option<String> {
    match expr {
        Expression::Bool(v, _) => {
            expr_entity_vec.push(ExprEntity::Val(ValType::Bool(*v)));
        }
        Expression::Integer(v) => {
            expr_entity_vec.push(ExprEntity::Val(ValType::Int(v.0 as i64)));
        }
        Expression::Float(v) => {
            expr_entity_vec.push(ExprEntity::Val(ValType::Float(v.0)));
        }
        Expression::Function(f, expr_vec, _) => {
            let fn_tuple = match f {
                sql_parse::Function::Other(name) => (name.to_string(), true),
                _ => (String::from("default func is not supported"), false),
            };
            let mut expr_sub_entity_vec = vec![];
            for expr in expr_vec {
                parse_expr(_func_generator, expr, record_id, &mut expr_sub_entity_vec);
            }
            if fn_tuple.1 {
                expr_entity_vec.push(ExprEntity::Function(fn_tuple.0, expr_sub_entity_vec));
            } else {
                return Some(fn_tuple.0);
            }
        }
        Expression::Identifier(id_vec) => {
            if id_vec.len() == 2 {
                let id_part1 = id_vec.first().unwrap();
                let id_part2 = id_vec.get(1).unwrap();
                let rs1 = fetch_record_id(record_id, id_part1);
                if rs1.is_err() {
                    return Some(rs1.err().unwrap());
                }
                let rs2 = fetch_field_id(record_id, id_part2);
                if rs2.is_err() {
                    return Some(rs2.err().unwrap());
                }
                let record_id = rs1.unwrap();
                let field_id = rs2.unwrap();
                expr_entity_vec.push(ExprEntity::FieldWithTab(record_id, field_id));
            } else if id_vec.len() == 1 {
                let id_part = id_vec.first().unwrap();
                let rs = fetch_field_id(record_id, id_part);
                if rs.is_err() {
                    return Some(rs.err().unwrap());
                }
                let field_id = rs.unwrap();
                expr_entity_vec.push(ExprEntity::Field(field_id));
            } else {
                return Some(String::from("id should be a valid identifier"));
            }
        }
        _ => {}
    }
    None
}

fn fetch_record_id(record_id: u16, id_part: &IdentifierPart<'_>) -> Result<u16, String> {
    match id_part {
        IdentifierPart::Name(id) => {
            let name = id.as_str();
            if let Some(id) = Record::fetch_record_id(name) {
                if id == record_id {
                    Ok(id)
                } else {
                    Err(format!(
                        "record [{name}] is not match the target record in sql query"
                    ))
                }
            } else {
                Err(format!("record [{name}] is not found"))
            }
        }
        IdentifierPart::Star(_) => Err(String::from("star is not supported")),
    }
}

fn fetch_field_id(record_id: u16, id_part: &IdentifierPart<'_>) -> Result<u16, String> {
    match id_part {
        IdentifierPart::Name(id) => {
            let name = id.as_str();
            if let Some(id) = Record::fetch_column_id(record_id, name) {
                Ok(*id)
            } else {
                Err(format!("field [{name}] is not found"))
            }
        }
        IdentifierPart::Star(_) => Err(String::from("star is not supported")),
    }
}
