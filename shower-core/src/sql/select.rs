use crate::{
    consts::DEFALUT_SELECT_SIZE,
    data::Record,
    sql::base::{parse_sql, ExprEntity, OpType, ParsedSql, ValType},
};
use sql_parse::{
    self, BinaryOperator, Expression, IdentifierPart, ParseOptions, Select, SelectExpr, Statement,
    TableReference,
};
use std::ops::Range;

pub(crate) fn parse_select(sql: &str, options: &ParseOptions) -> Option<ParsedSql> {
    parse_sql(sql, options, |ast| match ast {
        Statement::Select(stat) => parse_select_statement(stat),
        _ => None,
    })
}

fn parse_select_statement(select_stat: Select<'_>) -> Option<ParsedSql> {
    let mut issues = Vec::new();
    check_forbidden_statement(&select_stat, &mut issues);
    if issues.is_empty() {
        // 识别表名
        let records = parse_select_target(&select_stat.table_references, &mut issues);
        if check_record_invalid(&records) {
            return None;
        }
        let record = records[0];
        // 辨识过滤条件
        let filters = parse_select_condition(&select_stat.where_, record, &mut issues);
        // 限制数据范围
        let limit_range = parse_select_limitor(&select_stat.limit, &mut issues);
        // 辨识字段
        let fields = parse_select_fields(&select_stat.select_exprs, record, &mut issues);
        if issues.is_empty() {
            Some(ParsedSql::new(records, filters, limit_range, fields))
        } else {
            for issue in issues {
                log::warn!("hit issue: [{}]", issue);
            }
            None
        }
    } else {
        None
    }
}

fn check_record_invalid(records: &Vec<u16>) -> bool {
    if records.is_empty() {
        log::warn!("no records found in sql query");
        true
    } else if records.len() != 1 {
        log::warn!("not support multi record select, skipping");
        for record in records {
            log::warn!("record id in sql:{}", record);
        }
        true
    } else {
        false
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
        } => {
            if as_.is_some() {
                return Some(String::from("as is not supported"));
            }
            let name = identifier.identifier.value;
            if let Some(record_id) = Record::fetch_record_id(name) {
                tab_ref_vec.push(*record_id);
            } else {
                return Some(format!("record define: [{}] is not found", name));
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

fn parse_select_condition(
    where_: &Option<(Expression<'_>, Range<usize>)>,
    record_id: u16,
    issues: &mut Vec<String>,
) -> Vec<ExprEntity> {
    if let Some(where_part) = where_ {
        let mut expr_entity_vec = vec![];
        if let Some(issue) = parse_condition(&where_part.0, record_id, &mut expr_entity_vec) {
            issues.push(issue);
            vec![]
        } else {
            expr_entity_vec
        }
    } else {
        vec![]
    }
}

fn parse_select_limitor(
    limit: &Option<(Range<usize>, Option<Expression<'_>>, Expression<'_>)>,
    issues: &mut Vec<String>,
) -> usize {
    if let Some(limit_part) = limit {
        if limit_part.1.is_some() {
            issues.push(String::from("offset is not supported"));
            return 0;
        }
        match &limit_part.2 {
            Expression::Integer(v) => v.0 as usize,
            _ => DEFALUT_SELECT_SIZE,
        }
    } else {
        DEFALUT_SELECT_SIZE
    }
}

fn parse_select_fields(
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
        if let Some(value) = parse_expr(&select_expr.expr, record_id, &mut expr_vec) {
            issues.push(value);
            return vec![];
        }
    }
    expr_vec
}

fn parse_condition(
    expr: &Expression<'_>,
    record_id: u16,
    expr_entity_vec: &mut Vec<ExprEntity>,
) -> Option<String> {
    match expr {
        Expression::Binary {
            op,
            op_span: _,
            lhs,
            rhs,
        } => {
            if let Some(op_type) = conv_op(op) {
                expr_entity_vec.push(ExprEntity::Op(op_type));
            } else {
                return Some(format!("op[{:?}] is not supported", op));
            }
            let mut left_vec = vec![];
            if let Some(issue) = parse_condition(lhs.as_ref(), record_id, &mut left_vec) {
                return Some(issue);
            }
            expr_entity_vec.append(&mut left_vec);
            let mut right_vec = vec![];
            if let Some(issue) = parse_condition(rhs.as_ref(), record_id, &mut right_vec) {
                return Some(issue);
            }
            expr_entity_vec.append(&mut right_vec);
        }
        _ => {
            if let Some(issue) = check_not_allow_expr(expr) {
                return Some(issue);
            }
            if let Some(issue) = parse_expr(expr, record_id, expr_entity_vec) {
                return Some(issue);
            }
        }
    }
    None
}

fn check_not_support_op(op: &BinaryOperator) -> bool {
    matches!(
        op,
        BinaryOperator::Xor
            | BinaryOperator::NullSafeEq
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
            | BinaryOperator::BitAnd
            | BinaryOperator::BitOr
            | BinaryOperator::BitXor
            | BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Divide
            | BinaryOperator::Div
            | BinaryOperator::Mod
            | BinaryOperator::Mult
            | BinaryOperator::Like
            | BinaryOperator::NotLike
    )
}

fn conv_op(op: &BinaryOperator) -> Option<OpType> {
    if check_not_support_op(op) {
        None
    } else {
        match op {
            BinaryOperator::Or => Some(OpType::Or),
            BinaryOperator::And => Some(OpType::And),
            BinaryOperator::Eq => Some(OpType::Eq),
            BinaryOperator::GtEq => Some(OpType::GtEq),
            BinaryOperator::Gt => Some(OpType::Gt),
            BinaryOperator::LtEq => Some(OpType::LtEq),
            BinaryOperator::Lt => Some(OpType::Lt),
            BinaryOperator::Neq => Some(OpType::Neq),
            _ => None,
        }
    }
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

fn parse_expr(
    expr: &Expression<'_>,
    record_id: u16,
    expr_entity_vec: &mut Vec<ExprEntity>,
) -> Option<String> {
    match expr {
        Expression::Bool(v, _) => {
            expr_entity_vec.push(ExprEntity::Val(ValType::Bool(*v)));
        }
        Expression::String(v) => {
            expr_entity_vec.push(ExprEntity::Val(ValType::Str(v.value.to_string())));
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
                parse_expr(expr, record_id, &mut expr_sub_entity_vec);
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
                let rid = *id;
                if rid == record_id {
                    Ok(rid)
                } else {
                    Err(format!(
                        "record [{}] is not match the target record in sql query",
                        name
                    ))
                }
            } else {
                Err(format!("record [{}] is not found", name))
            }
        }
        IdentifierPart::Star(_) => Err(String::from("star is not supported")),
    }
}

fn fetch_field_id(record_id: u16, id_part: &IdentifierPart<'_>) -> Result<u16, String> {
    match id_part {
        IdentifierPart::Name(id) => {
            let name = id.as_str();
            if let Some(id) = Record::fetch_field_id(record_id, name) {
                Ok(*id)
            } else {
                Err(format!("field [{}] is not found", name))
            }
        }
        IdentifierPart::Star(_) => Err(String::from("star is not supported")),
    }
}
