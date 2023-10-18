use crate::{
    error::ActionError,
    sql::{ExprEntity, OpType, ParsedSql},
    store::{self, DataIterator},
};
use std::str::FromStr;
use strum_macros::EnumString;

pub(crate) fn gen_action<'a>(parsed_sql: &ParsedSql) -> Result<Action<'a>, ActionError> {
    let tab_id_array = &parsed_sql.tables();
    if tab_id_array.len() != 1 {
        return Err(ActionError::new(format!(
            "only support one table, but {} tables",
            tab_id_array.len()
        )));
    }
    let id = *tab_id_array.first().unwrap();
    let iterator = create_iterator(id);
    let rs_filter = create_filter(&parsed_sql.filters());
    if rs_filter.is_err() {
        return Err(ActionError::new(format!(
            "failed to parse filter: {}",
            rs_filter.err().unwrap()
        )));
    }
    let rs_executors = create_executor(tab_id_array, &parsed_sql.fields());
    if rs_executors.is_err() {
        return Err(ActionError::new(format!(
            "failed to parse executors: {}",
            rs_executors.err().unwrap()
        )));
    }
    Ok(Action {
        id,
        iterator,
        filter: rs_filter.unwrap(),
        limit: parsed_sql.limit(),
        _executors: rs_executors.unwrap(),
    })
}

fn create_iterator<'a>(id: u16) -> DataIterator<'a> {
    store::create_iterator(id)
}

fn create_filter(entities: &[ExprEntity]) -> Result<Filter, ActionError> {
    let mut filters: Vec<Filter> = entities
        .iter()
        .map(|e| Filter::Original(e.clone()))
        .collect();
    filters.reverse();
    let mut tmp: Vec<Filter> = vec![];
    loop {
        if filters.is_empty() {
            break;
        }
        let filter = filters.remove(0);
        let is_op = match &filter {
            Filter::Original(expr) => matches!(expr, ExprEntity::Op(_)),
            Filter::Mixed(_) => false,
        };
        if is_op {
            let m2 = tmp.pop().unwrap();
            let m1 = tmp.pop().unwrap();
            tmp.push(Filter::Mixed(vec![m1, m2, filter]));
        } else {
            tmp.push(filter);
        }
    }
    Ok(tmp.first().unwrap().clone())
}

fn create_executor(
    tab_id_array: &Vec<u16>,
    entities: &Vec<ExprEntity>,
) -> Result<Vec<Executor>, ActionError> {
    let mut executors = vec![];
    for entity in entities {
        let rs = conv_as_executor(entity, tab_id_array);
        if let Ok(executor) = rs {
            executors.push(executor);
        } else {
            return Err(rs.err().unwrap());
        }
    }
    Ok(executors)
}

fn conv_as_executor(entity: &ExprEntity, tab_id_array: &Vec<u16>) -> Result<Executor, ActionError> {
    let fetcher = match entity {
        ExprEntity::Field(field_id) => Executor::Fetch(*tab_id_array.first().unwrap(), *field_id),
        ExprEntity::FieldWithTab(tab_id, field_id) => Executor::Fetch(*tab_id, *field_id),
        ExprEntity::Function(func_name, args) => {
            if func_name.starts_with('_') {
                let mut real_func_name = func_name.clone();
                real_func_name.remove(0);
                let opt_func = Func::from_str(real_func_name.as_str());
                if let Ok(func) = opt_func {
                    let rs = parse_args_fetchers(args, tab_id_array);
                    if let Ok(args_fetchers) = rs {
                        Executor::Compute(func, args_fetchers)
                    } else {
                        return Err(rs.err().unwrap());
                    }
                } else {
                    return Err(ActionError::new(format!("unknown func: {:?}", func_name)));
                }
            } else {
                return Err(ActionError::new(format!("unknown func: {:?}", func_name)));
            }
        }
        _ => {
            return Err(ActionError::new(format!(
                "cannot hit this case: {:?}",
                entity
            )));
        }
    };
    Ok(fetcher)
}

fn parse_args_fetchers(
    args: &Vec<ExprEntity>,
    tab_id_array: &Vec<u16>,
) -> Result<Vec<Executor>, ActionError> {
    let mut args_fetchers = vec![];
    let mut hit_error = false;
    args.iter()
        .map(|arg| conv_as_executor(arg, tab_id_array))
        .for_each(|e| {
            if let Ok(fetcher) = e {
                args_fetchers.push(fetcher);
            } else {
                hit_error = true;
            }
        });
    if hit_error {
        Err(ActionError::new(format!(
            "failed to parse args: {:?}",
            args
        )))
    } else {
        Ok(args_fetchers)
    }
}

pub(crate) fn invoke(action: &Action) {
    let _: Vec<_> = action
        .iterator
        .filter(|e| action.filter().is_match(e))
        .take(action.limit)
        .map(|_e| {})
        .collect();
}

fn op_or(slice: &[u8], v1: &Filter, v2: &Filter) -> bool {
    match (v1, v2) {
        (Filter::Mixed(_), Filter::Mixed(_)) => v1.is_match(slice) || v2.is_match(slice),
        _ => false,
    }
}
fn op_and(slice: &[u8], v1: &Filter, v2: &Filter) -> bool {
    match (v1, v2) {
        (Filter::Mixed(_), Filter::Mixed(_)) => v1.is_match(slice) && v2.is_match(slice),
        _ => false,
    }
}
fn op_eq(slice: &[u8], v1: &Filter, v2: &Filter) -> bool {
    log::info!("eq, p[{:?}], v1:[{:?}], v2:[{:?}]", slice.as_ptr(), v1, v2);
    match (v1, v2) {
        (Filter::Original(expr1), Filter::Original(expr2)) => match (expr1, expr2) {
            (ExprEntity::Val(_), ExprEntity::Field(_)) => {
                log::info!("eq, v, f");
                true
            }
            (ExprEntity::Val(_), ExprEntity::FieldWithTab(_, _)) => {
                log::info!("eq, v, fwt");
                true
            }
            (ExprEntity::Val(_), ExprEntity::Function(_, _)) => {
                log::info!("eq, v, fn");
                true
            }
            (ExprEntity::Field(_), ExprEntity::Val(_)) => {
                log::info!("eq, f, v");
                true
            }
            (ExprEntity::FieldWithTab(_, _), ExprEntity::Val(_)) => {
                log::info!("eq, fwt, v");
                true
            }
            (ExprEntity::Function(_, _), ExprEntity::Val(_)) => {
                log::info!("eq, fn, v");
                true
            }
            _ => false,
        },
        _ => false,
    }
}
fn op_gt_eq(slice: &[u8], _v1: &Filter, _v2: &Filter) -> bool {
    log::info!("gt_eq, p[{:?}]", slice.as_ptr());
    true
}
fn op_gt(slice: &[u8], _v1: &Filter, _v2: &Filter) -> bool {
    log::info!("gt, p[{:?}]", slice.as_ptr());
    true
}
fn op_lt_eq(slice: &[u8], _v1: &Filter, _v2: &Filter) -> bool {
    log::info!("lt_eq, p[{:?}]", slice.as_ptr());
    true
}
fn op_lt(slice: &[u8], _v1: &Filter, _v2: &Filter) -> bool {
    log::info!("lt, p[{:?}]", slice.as_ptr());
    true
}
fn op_neq(slice: &[u8], _v1: &Filter, _v2: &Filter) -> bool {
    log::info!("neq, p[{:?}]", slice.as_ptr());
    true
}

#[derive(Debug)]
pub(crate) struct Action<'a> {
    id: u16,
    iterator: DataIterator<'a>,
    filter: Filter,
    limit: usize,
    _executors: Vec<Executor>,
}
impl<'a> Action<'a> {
    pub(crate) fn id(&self) -> u16 {
        self.id
    }
    pub(crate) fn _iterator(&self) -> &DataIterator<'a> {
        &self.iterator
    }
    pub(crate) fn filter(&self) -> &Filter {
        &self.filter
    }
    pub(crate) fn _limit(&self) -> usize {
        self.limit
    }
    pub(crate) fn _executors(&self) -> &Vec<Executor> {
        &self._executors
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Filter {
    Original(ExprEntity),
    Mixed(Vec<Filter>),
}
impl Filter {
    fn is_match(&self, slice: &[u8]) -> bool {
        match self {
            Filter::Original(_) => false,
            Filter::Mixed(filters) => self.filter_slice(filters, slice),
        }
    }

    fn filter_slice(&self, filters: &[Filter], slice: &[u8]) -> bool {
        let v1 = &filters[0];
        let v2 = &filters[1];
        let op = &filters[2];
        match op {
            Filter::Original(expr) => self.filter_slice_by_expr(expr, v1, v2, slice),
            Filter::Mixed(_) => false,
        }
    }

    fn filter_slice_by_expr(
        &self,
        expr: &ExprEntity,
        v1: &Filter,
        v2: &Filter,
        slice: &[u8],
    ) -> bool {
        match expr {
            ExprEntity::Op(op_type) => match op_type {
                OpType::Or => op_or(slice, v1, v2),
                OpType::And => op_and(slice, v1, v2),
                OpType::Eq => op_eq(slice, v1, v2),
                OpType::GtEq => op_gt_eq(slice, v1, v2),
                OpType::Gt => op_gt(slice, v1, v2),
                OpType::LtEq => op_lt_eq(slice, v1, v2),
                OpType::Lt => op_lt(slice, v1, v2),
                OpType::Neq => op_neq(slice, v1, v2),
            },
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Executor {
    Fetch(u16, u16),
    Compute(Func, Vec<Executor>),
}
#[derive(Debug, Clone, EnumString)]
pub(crate) enum Func {
    #[strum(ascii_case_insensitive)]
    Add,
    #[strum(ascii_case_insensitive)]
    Sub,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cfg::{get_config, load_config},
        data::U8Bytes,
        logger::init_logger,
        sql::{parse_options, parse_sql},
        store::insert,
    };
    use std::env;

    const SQL: &str = r#"
    SELECT _1.__1, _1.__2, _1.__3, _sub(_add(_1.__4, _1.__4), _1.__5)
    FROM _1
    WHERE (_1.__1 = '1' AND _1.__2 = '2') OR (_1.__1 = '3' AND _1.__2 = '4')
    LIMIT 10
    "#;

    #[test]
    fn test_action() {
        env::set_var("SHOWER_HOME", "/home/helly/code/rust/shower");
        load_config();
        init_logger(get_config());
        let mut vec = vec![0u8; 288];
        vec.as_mut_slice()[3] = 1;
        let u8data = U8Bytes::new_from_vec(1, 288, vec);
        for _ in 0..100 {
            insert(&u8data);
        }
        let parse_options = parse_options();
        if let Some(parsed_sql) = parse_sql(SQL, &parse_options) {
            let rs_action = gen_action(&parsed_sql);
            if let Ok(action) = rs_action {
                invoke(&action);
            }
        }
    }
}
