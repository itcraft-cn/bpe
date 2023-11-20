use sql_parse::{parse_statement, ParseOptions, SQLArguments, SQLDialect, Statement};

pub(crate) fn parse_options() -> ParseOptions {
    ParseOptions::new()
        .dialect(SQLDialect::MariaDB)
        .arguments(SQLArguments::QuestionMark)
        .warn_unquoted_identifiers(false)
}

pub(crate) fn parse_sql<F>(sql: &str, options: &ParseOptions, f: F) -> Option<ParsedSql>
where
    F: Fn(Statement<'_>) -> Option<ParsedSql>,
{
    let ast_opt = parse_sql_statement(sql, options);
    if let Some(ast) = ast_opt {
        let opt = f(ast);
        if opt.is_none() {
            log::warn!("failed to parse sql [{}]", sql);
        }
        opt
    } else {
        log::warn!("failed to parse sql [{}]", sql);
        None
    }
}

pub(crate) fn parse_sql_statement<'a>(
    sql: &'a str,
    options: &ParseOptions,
) -> Option<Statement<'a>> {
    let mut issues = Vec::new();
    let ast_opt = parse_statement(sql, &mut issues, options);
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
    ast_opt
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
    records: Vec<u16>,
    filters: Vec<ExprEntity>,
    limit: usize,
    fields: Vec<ExprEntity>,
}
impl ParsedSql {
    pub(crate) fn new(
        records: Vec<u16>,
        filters: Vec<ExprEntity>,
        limit: usize,
        fields: Vec<ExprEntity>,
    ) -> ParsedSql {
        ParsedSql {
            records,
            filters,
            limit,
            fields,
        }
    }

    pub(crate) fn records(&self) -> &Vec<u16> {
        &self.records
    }

    pub(crate) fn filters(&self) -> &Vec<ExprEntity> {
        &self.filters
    }

    pub(crate) fn limit(&self) -> usize {
        self.limit
    }

    pub(crate) fn fields(&self) -> &Vec<ExprEntity> {
        &self.fields
    }
}
