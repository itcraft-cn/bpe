use crate::{
    error::ActionError,
    func::{eq, fetch_val, gt, gt_eq, lt, lt_eq, neq, Executor, FnHolder, Func},
    sql::{ExprEntity, OpType, ParsedSql, ValType},
    store::{self, DataIterator},
};
use std::str::FromStr;

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
        executors: rs_executors.unwrap(),
    })
}

fn create_iterator<'a>(id: u16) -> DataIterator<'a> {
    store::create_iterator(id)
}

fn create_filter(entities: &[ExprEntity]) -> Result<Filter, ActionError> {
    if entities.len() == 0 {
        return Ok(Filter::Empty);
    }
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
            Filter::Empty => false,
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
    if tmp.len() == 1 {
        Ok(tmp.first().unwrap().clone())
    } else {
        Err(ActionError::new(
            "the last element is not found.".to_owned(),
        ))
    }
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

pub(crate) fn invoke(action: &Action, fn_holder: &FnHolder) {
    let vec: Vec<[u64; 64]> = action
        .iterator
        .filter(|slice| action.filter.is_match(slice))
        .take(action.limit)
        .map(|slice| action.fetch(slice))
        .collect();
    match fn_holder {
        FnHolder::NotExist => {}
        FnHolder::Func(f) => f(vec),
        FnHolder::FfiFunc(ffi) => ffi.callback(vec),
    }
}

#[inline]
fn op_or(slice: &[u8], v1: &Filter, v2: &Filter) -> bool {
    op_logical(slice, v1, v2, or)
}
#[inline]
fn op_and(slice: &[u8], v1: &Filter, v2: &Filter) -> bool {
    op_logical(slice, v1, v2, and)
}
#[inline]
fn op_logical<F>(slice: &[u8], v1: &Filter, v2: &Filter, f: F) -> bool
where
    F: Fn(&[u8], &Filter, &Filter) -> bool,
{
    match (v1, v2) {
        (Filter::Mixed(_), Filter::Mixed(_)) => f(slice, v1, v2),
        _ => false,
    }
}

#[inline]
fn or(slice: &[u8], v1: &Filter, v2: &Filter) -> bool {
    v1.is_match(slice) || v2.is_match(slice)
}

#[inline]
fn and(slice: &[u8], v1: &Filter, v2: &Filter) -> bool {
    v1.is_match(slice) && v2.is_match(slice)
}

#[inline]
fn op_eq(slice: &[u8], v1: &Filter, v2: &Filter) -> bool {
    compare_slice_val(slice, v1, v2, eq)
}
#[inline]
fn op_gt_eq(slice: &[u8], v1: &Filter, v2: &Filter) -> bool {
    compare_slice_val(slice, v1, v2, gt_eq)
}
#[inline]
fn op_gt(slice: &[u8], v1: &Filter, v2: &Filter) -> bool {
    compare_slice_val(slice, v1, v2, gt)
}
#[inline]
fn op_lt_eq(slice: &[u8], v1: &Filter, v2: &Filter) -> bool {
    compare_slice_val(slice, v1, v2, lt_eq)
}
#[inline]
fn op_lt(slice: &[u8], v1: &Filter, v2: &Filter) -> bool {
    compare_slice_val(slice, v1, v2, lt)
}
#[inline]
fn op_neq(slice: &[u8], v1: &Filter, v2: &Filter) -> bool {
    compare_slice_val(slice, v1, v2, neq)
}

#[inline]
fn compare_slice_val<F>(slice: &[u8], v1: &Filter, v2: &Filter, f: F) -> bool
where
    F: Fn(u64, u64) -> bool,
{
    match (v1, v2) {
        (Filter::Original(expr1), Filter::Original(expr2)) => match (expr1, expr2) {
            (ExprEntity::Field(idx), ExprEntity::Val(v_type)) => {
                compare_with_op(slice, *idx, v_type, f)
            }
            (ExprEntity::FieldWithTab(_, field_idx), ExprEntity::Val(v_type)) => {
                compare_with_op(slice, *field_idx, v_type, f)
            }
            (ExprEntity::Val(v_type), ExprEntity::Field(idx)) => {
                compare_with_op(slice, *idx, v_type, f)
            }
            (ExprEntity::Val(v_type), ExprEntity::FieldWithTab(_, field_idx)) => {
                compare_with_op(slice, *field_idx, v_type, f)
            }
            _ => false,
        },
        _ => false,
    }
}

#[inline]
fn compare_with_op<F>(slice: &[u8], idx: u16, v_type: &ValType, f: F) -> bool
where
    F: Fn(u64, u64) -> bool,
{
    let opt_expacted = fetch_expacted(v_type);
    if let Some(expacted) = opt_expacted {
        let val = fetch_val(slice, idx);
        compare_val(expacted, val, f)
    } else {
        false
    }
}

#[inline]
fn fetch_expacted(v_type: &ValType) -> Option<u64> {
    match v_type {
        ValType::Int(val) => Some(*val as u64),
        _ => None,
    }
}

#[inline]
fn compare_val<F>(expacted: u64, val: u64, f: F) -> bool
where
    F: Fn(u64, u64) -> bool,
{
    f(expacted, val)
}

#[derive(Debug)]
pub(crate) struct Action<'a> {
    id: u16,
    iterator: DataIterator<'a>,
    filter: Filter,
    limit: usize,
    executors: Vec<Executor>,
}
impl<'a> Action<'a> {
    pub(crate) fn id(&self) -> u16 {
        self.id
    }

    fn fetch(&self, slice: &[u8]) -> [u64; 64] {
        let mut result = [0u64; 64];
        for (i, executor) in self.executors.iter().enumerate() {
            result[i] = executor.fetch(slice);
        }
        result
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Filter {
    Empty,
    Original(ExprEntity),
    Mixed(Vec<Filter>),
}
impl Filter {
    fn is_match(&self, slice: &[u8]) -> bool {
        match self {
            Filter::Empty => true,
            Filter::Original(_) => false,
            Filter::Mixed(filters) => self.filter_slice(filters, slice),
        }
    }

    fn filter_slice(&self, filters: &[Filter], slice: &[u8]) -> bool {
        let v1 = &filters[0];
        let v2 = &filters[1];
        let op = &filters[2];
        match op {
            Filter::Empty => true,
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
                invoke(&action, &FnHolder::NotExist);
            }
        }
    }
}
