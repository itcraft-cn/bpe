use crate::consts::DEFALUT_SELECT_SIZE;
use sql_parse::{
    self, BinaryOperator, Expression, IdentifierPart, ParseOptions, SQLArguments, SQLDialect,
    Select, SelectExpr, Statement, TableReference,
};
use std::ops::Range;

pub(crate) fn parse_options() -> ParseOptions {
    ParseOptions::new()
        .dialect(SQLDialect::MariaDB)
        .arguments(SQLArguments::QuestionMark)
        .warn_unquoted_identifiers(false)
}

pub(crate) fn parse_sql(sql: &str, options: &ParseOptions) -> Option<ParsedSql> {
    let mut issues = Vec::new();
    let ast_opt = sql_parse::parse_statement(sql, &mut issues, options);
    if !issues.is_empty() {
        for issue in &issues {
            log::warn!(
                "found issue: [{:?}]{:?} at {:?}",
                issue.level,
                issue.message,
                issue.span
            );
            for fragment in &issue.fragments {
                log::warn!("detail: {:?}, {:?}", fragment.0, fragment.1);
            }
        }
        return None;
    }
    if let Some(ast) = ast_opt {
        match ast {
            Statement::Select(stat) => {
                let mut issues2 = Vec::new();
                parse_select_statement(stat, &mut issues2)
            }
            _ => None,
        }
    } else {
        None
    }
}

fn parse_select_statement(select_stat: Select<'_>, issues: &mut Vec<String>) -> Option<ParsedSql> {
    check_forbidden_statement(&select_stat, issues);
    if issues.is_empty() {
        // 识别表名
        let tables = parse_select_target(&select_stat.table_references, issues);
        // 辨识过滤条件
        let filters = parse_select_condition(&select_stat.where_, issues);
        // 限制数据范围
        let limit_range = parse_select_limitor(&select_stat.limit, issues);
        // 辨识字段
        let fields = parse_select_fields(&select_stat.select_exprs, issues);
        if issues.is_empty() {
            Some(ParsedSql::new(tables, filters, limit_range, fields))
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
    table_references: &Option<Vec<TableReference<'_>>>,
    issues: &mut Vec<String>,
) -> Vec<u16> {
    if let Some(tab_vec) = table_references {
        let mut tab_ref_vec = vec![];
        for tab in tab_vec {
            if let Some(value) = parse_tab_ref(tab, &mut tab_ref_vec) {
                issues.push(value);
                return vec![];
            }
        }
        tab_ref_vec
    } else {
        issues.push(String::from("need a table name"));
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
            for id in identifier {
                if !id.starts_with('_') {
                    return Some(String::from("should be a valid identifier, start with `_`"));
                }
                let rs = conv_tab_id(String::from(id.value));
                let tab_id = if let Ok(tid) = rs {
                    tid
                } else {
                    return Some(rs.err().unwrap());
                };
                tab_ref_vec.push(tab_id);
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
    issues: &mut Vec<String>,
) -> Vec<ExprEntity> {
    if let Some(where_part) = where_ {
        let mut expr_entity_vec = vec![];
        if let Some(issue) = parse_condition(&where_part.0, &mut expr_entity_vec) {
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
        if let Some(value) = parse_expr(&select_expr.expr, &mut expr_vec) {
            issues.push(value);
            return vec![];
        }
    }
    expr_vec
}

fn parse_condition(expr: &Expression<'_>, expr_entity_vec: &mut Vec<ExprEntity>) -> Option<String> {
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
            if let Some(issue) = parse_condition(lhs.as_ref(), &mut left_vec) {
                return Some(issue);
            }
            expr_entity_vec.append(&mut left_vec);
            let mut right_vec = vec![];
            if let Some(issue) = parse_condition(rhs.as_ref(), &mut right_vec) {
                return Some(issue);
            }
            expr_entity_vec.append(&mut right_vec);
        }
        _ => {
            if let Some(issue) = check_not_allow_expr(expr) {
                return Some(issue);
            }
            if let Some(issue) = parse_expr(expr, expr_entity_vec) {
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

fn parse_expr(expr: &Expression<'_>, expr_entity_vec: &mut Vec<ExprEntity>) -> Option<String> {
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
                parse_expr(expr, &mut expr_sub_entity_vec);
            }
            if fn_tuple.1 {
                expr_entity_vec.push(ExprEntity::Function(fn_tuple.0, expr_sub_entity_vec));
            } else {
                return Some(fn_tuple.0);
            }
        }
        Expression::Identifier(id_vec) => {
            if id_vec.len() == 2 {
                let id_part1 = id_vec.get(0).unwrap();
                let id_part2 = id_vec.get(1).unwrap();
                let converted_id_part1 = fetch_id_part(id_part1);
                if !converted_id_part1.1 {
                    return Some(converted_id_part1.0);
                }
                let converted_id_part2 = fetch_id_part(id_part2);
                if !converted_id_part2.1 {
                    return Some(converted_id_part2.0);
                }
                let rs_tab_id = conv_tab_id(converted_id_part1.0);
                let tab_id = if let Ok(id) = rs_tab_id {
                    id
                } else {
                    return Some(rs_tab_id.err().unwrap());
                };
                let rs_field_id = conv_field_id(converted_id_part2.0);
                let field_id = if let Ok(id) = rs_field_id {
                    id
                } else {
                    return Some(rs_field_id.err().unwrap());
                };
                expr_entity_vec.push(ExprEntity::FieldWithTab(tab_id, field_id));
            } else if id_vec.len() == 1 {
                let id_part = id_vec.get(0).unwrap();
                let converted_id_part = fetch_id_part(id_part);
                if !converted_id_part.1 {
                    return Some(converted_id_part.0);
                }
                let rs_field_id = conv_field_id(converted_id_part.0);
                let field_id = if let Ok(id) = rs_field_id {
                    id
                } else {
                    return Some(rs_field_id.err().unwrap());
                };
                expr_entity_vec.push(ExprEntity::Field(field_id));
            } else {
                return Some(String::from("id should be a valid identifier"));
            }
        }
        _ => {}
    }
    None
}

fn conv_tab_id(id: String) -> Result<u16, String> {
    conv_id(id, 1)
}

fn conv_field_id(id: String) -> Result<u16, String> {
    conv_id(id, 2)
}

fn conv_id(id: String, idx: usize) -> Result<u16, String> {
    let v_str = String::from_utf8(id.as_bytes()[idx..].to_vec()).unwrap();
    let o = v_str.parse::<u16>();
    if let Ok(v) = o {
        if v > 0 {
            Ok(v)
        } else {
            Err(String::from("id should be a positive integer"))
        }
    } else {
        Err(format!("hit error: {}", o.unwrap_err()))
    }
}

fn fetch_id_part(id_part: &IdentifierPart<'_>) -> (String, bool) {
    match id_part {
        IdentifierPart::Name(id) => {
            if !id.starts_with("__") && !id.starts_with('_') {
                (
                    String::from("should be a valid identifier, start with `_` or `__`"),
                    false,
                )
            } else {
                (String::from(id.value), true)
            }
        }
        IdentifierPart::Star(_) => (String::from("star is not supported"), false),
    }
}

#[derive(Debug, Clone)]
pub(crate) enum ExprEntity {
    Op(OpType),
    Val(ValType),
    Field(u16),
    FieldWithTab(u16, u16),
    Function(String, Vec<ExprEntity>),
}
#[derive(Debug, Clone)]
pub(crate) enum ValType {
    Bool(bool),
    Str(String),
    Int(i64),
    Float(f64),
}
#[derive(Debug, Clone)]
pub(crate) enum OpType {
    Or,
    And,
    Eq,
    GtEq,
    Gt,
    LtEq,
    Lt,
    Neq,
}

#[derive(Debug, Clone)]
pub(crate) struct ParsedSql {
    tables: Vec<u16>,
    filters: Vec<ExprEntity>,
    limit: usize,
    fields: Vec<ExprEntity>,
}
impl ParsedSql {
    pub(crate) fn new(
        tables: Vec<u16>,
        filters: Vec<ExprEntity>,
        limit: usize,
        fields: Vec<ExprEntity>,
    ) -> ParsedSql {
        ParsedSql {
            tables,
            filters,
            limit,
            fields,
        }
    }

    pub(crate) fn tables(&self) -> Vec<u16> {
        self.tables.clone()
    }

    pub(crate) fn filters(&self) -> Vec<ExprEntity> {
        self.filters.clone()
    }

    pub(crate) fn limit(&self) -> usize {
        self.limit
    }

    pub(crate) fn fields(&self) -> Vec<ExprEntity> {
        self.fields.clone()
    }
}
