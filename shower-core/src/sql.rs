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
                let opt = parse_select_statement(stat, &mut issues2);
                opt
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
        if issues.is_empty(){
            Some(ParsedSql::new(tables, filters, limit_range, fields))
        } else {
            for issue in issues{
                log::warn!("hit issue: [{}]",issue);
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
        return;
    }
    if select_stat.locking.is_some() {
        issues.push(String::from("locking statement is not supported"));
        return;
    }
    if select_stat.having.is_some() {
        issues.push(String::from("having statement is not supported"));
        return;
    }
    if select_stat.group_by.is_some() {
        issues.push(String::from("group by statement is not supported"));
        return;
    }
    if select_stat.order_by.is_some() {
        issues.push(String::from("order by statement is not supported"));
        return;
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
        return vec![];
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
                if !id.starts_with("_") {
                    return Some(String::from("should be a valid identifier, start with `_`"));
                }
                tab_ref_vec.push(conv_tab_id(String::from(id.value)));
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
            if check_op(op) {
                return Some(format!("op[{:?}] is not supported", op));
            } else {
                if let Some(op_type) = conv_op(op) {
                    expr_entity_vec.push(ExprEntity::Op(op_type));
                } else {
                    return Some(format!("op[{:?}] is not supported", op));
                }
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

fn check_op(op: &BinaryOperator) -> bool {
    match op {
        BinaryOperator::Xor => true,
        BinaryOperator::NullSafeEq => true,
        BinaryOperator::ShiftLeft => true,
        BinaryOperator::ShiftRight => true,
        BinaryOperator::BitAnd => true,
        BinaryOperator::BitOr => true,
        BinaryOperator::BitXor => true,
        BinaryOperator::Add => true,
        BinaryOperator::Subtract => true,
        BinaryOperator::Divide => true,
        BinaryOperator::Div => true,
        BinaryOperator::Mod => true,
        BinaryOperator::Mult => true,
        BinaryOperator::Like => true,
        BinaryOperator::NotLike => true,
        _ => false,
    }
}

fn conv_op(op: &BinaryOperator) -> Option<OpType> {
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

fn check_not_allow_expr(expr: &Expression<'_>) -> Option<String> {
    let not_allow = match &expr {
        Expression::Subquery(_) => true,
        Expression::Null(_) => true,
        Expression::ListHack(_) => true,
        Expression::WindowFunction {
            function: _,
            args: _,
            function_span: _,
            over_span: _,
            window_spec: _,
        } => true,
        Expression::Arg(_) => true,
        Expression::Exists(_) => true,
        Expression::In {
            lhs: _,
            rhs: _,
            in_span: _,
            not_in: _,
        } => true,
        Expression::Is(_, _, _) => true,
        Expression::Invalid(_) => true,
        Expression::Case {
            case_span: _,
            value: _,
            whens: _,
            else_: _,
            end_span: _,
        } => true,
        Expression::Cast {
            cast_span: _,
            expr: _,
            as_span: _,
            type_: _,
        } => true,
        Expression::Count {
            count_span: _,
            distinct_span: _,
            expr: _,
        } => true,
        Expression::GroupConcat {
            group_concat_span: _,
            distinct_span: _,
            expr: _,
        } => true,
        Expression::Variable {
            global: _,
            session: _,
            dot: _,
            variable: _,
            variable_span: _,
        } => true,
        Expression::Unary {
            op: _,
            op_span: _,
            operand: _,
        } => true,
        _ => false,
    };
    if not_allow {
        Some(format!("expression {:?} is not supported", &expr))
    } else {
        None
    }
}

fn parse_expr(expr: &Expression<'_>, expr_entity_vec: &mut Vec<ExprEntity>) -> Option<String> {
    match expr {
        Expression::Bool(v, _) => {
            expr_entity_vec.push(ExprEntity::Val(ValType::Bool(v.clone())));
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
                expr_entity_vec.push(ExprEntity::FieldWithTab(
                    conv_tab_id(converted_id_part1.0),
                    conv_field_id(converted_id_part2.0),
                ));
            } else if id_vec.len() == 1 {
                let id_part = id_vec.get(0).unwrap();
                let converted_id_part = fetch_id_part(id_part);
                if !converted_id_part.1 {
                    return Some(converted_id_part.0);
                }
                expr_entity_vec.push(ExprEntity::Field(conv_field_id(converted_id_part.0)));
            } else {
                return Some(String::from("id should be a valid identifier"));
            }
        }
        _ => {}
    }
    None
}

fn conv_tab_id(id: String) -> u16 {
    conv_id(id, 1)
}

fn conv_field_id(id: String) -> u16 {
    conv_id(id, 2)
}

fn conv_id(id: String, idx: usize) -> u16 {
    let v_str = String::from_utf8(id.as_bytes()[idx..].to_vec()).unwrap();
    v_str.parse::<u16>().unwrap()
}

fn fetch_id_part(id_part: &IdentifierPart<'_>) -> (String, bool) {
    match id_part {
        IdentifierPart::Name(id) => {
            if !id.starts_with("__") && !id.starts_with("_") {
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
    _limit_range: usize,
    fields: Vec<ExprEntity>,
}
impl ParsedSql {
    pub(crate) fn new(
        tables: Vec<u16>,
        filters: Vec<ExprEntity>,
        limit_range: usize,
        fields: Vec<ExprEntity>,
    ) -> ParsedSql {
        ParsedSql {
            tables,
            filters,
            _limit_range: limit_range,
            fields,
        }
    }

    pub fn tables(&self) -> Vec<u16> {
        self.tables.clone()
    }

    pub(crate) fn filters(&self) -> Vec<ExprEntity> {
        self.filters.clone()
    }

    pub(crate) fn fields(&self) -> Vec<ExprEntity> {
        self.fields.clone()
    }
}
