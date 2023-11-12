use crate::{
    aux::SimpleU16Map,
    data::U8Bytes,
    define::{get_define, Column},
    element::Element,
    error::ActionError,
    func::{eq, fetch_val, gt, gt_eq, lt, lt_eq, neq, Executor, FnHolder, Func},
    sql::parse_options,
    sql::{parse_sql, ExprEntity, OpType, ParsedSql, ValType},
    store::{self, DataIterator},
};
use sql_parse::ParseOptions;
use std::str::FromStr;

static mut ACTION_MAP: Option<SimpleU16Map<Vec<(Action, FnHolder)>>> = None;
static mut PARSE_OPTIONS: Option<ParseOptions> = None;

pub(crate) fn init_action_store() {
    unsafe {
        PARSE_OPTIONS = Some(parse_options());
        ACTION_MAP = Some(SimpleU16Map::new());
    }
}

pub(crate) fn define_action(sql: &str, func_holder: FnHolder) -> bool {
    let opt_parsed_sql = parse_sql(sql, unsafe { PARSE_OPTIONS.as_ref().unwrap() });
    if let Some(parsed_sql) = opt_parsed_sql {
        let rs = gen_action(&parsed_sql);
        if let Ok(action) = rs {
            let id = action.id();
            let map = unsafe { ACTION_MAP.as_mut().unwrap() };
            map.entry(id).or_insert_with(map, Vec::new);
            map.get_mut(id).push((action, func_holder));
            true
        } else {
            log::warn!(
                "fail to create action from sql[{}], hit unexpected error: {:?}",
                sql,
                rs.err().unwrap()
            );
            false
        }
    } else {
        log::warn!("not supported sql statement: [{}]", sql);
        false
    }
}

#[inline]
pub(crate) fn call_action(data: &U8Bytes) {
    let id = data.id();
    let opt_actions = search_aciton(id);
    let opt_defines = get_define(id);
    if opt_actions.is_none() || opt_defines.is_none() {
        return;
    }
    let actions = opt_actions.unwrap();
    let defines = opt_defines.unwrap();
    for (action, fn_holder) in actions {
        invoke(id, action, defines, fn_holder);
    }
}

fn search_aciton<'a>(id: u16) -> Option<&'a Vec<(Action<'a>, FnHolder)>> {
    let map = unsafe { ACTION_MAP.as_ref().unwrap() };
    map.get(id)
}

fn gen_action<'a>(parsed_sql: &ParsedSql) -> Result<Action<'a>, ActionError> {
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
    if entities.is_empty() {
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

fn invoke(id: u16, action: &'static Action, defines: &Vec<Column>, fn_holder: &FnHolder) {
    let vec: Vec<[u8; 512]> = action
        .iterator
        .filter(|slice| action.filter.is_match(id, defines, slice))
        .take(action.limit)
        .map(|slice| action.fetch(id, defines, slice))
        .collect();
    match fn_holder {
        FnHolder::NotExist => {}
        FnHolder::Func(f) => f(vec),
        FnHolder::FfiFunc(ffi) => ffi.callback(vec),
    }
}

#[inline]
fn op_or(slice: &'static [u8], id: u16, defines: &Vec<Column>, v1: &Filter, v2: &Filter) -> bool {
    op_logical(slice, id, defines, v1, v2, or)
}
#[inline]
fn op_and(slice: &'static [u8], id: u16, defines: &Vec<Column>, v1: &Filter, v2: &Filter) -> bool {
    op_logical(slice, id, defines, v1, v2, and)
}
#[inline]
fn op_logical<F>(
    slice: &'static [u8],
    id: u16,
    defines: &Vec<Column>,
    v1: &Filter,
    v2: &Filter,
    f: F,
) -> bool
where
    F: Fn(&'static [u8], u16, &Vec<Column>, &Filter, &Filter) -> bool,
{
    match (v1, v2) {
        (Filter::Mixed(_), Filter::Mixed(_)) => f(slice, id, defines, v1, v2),
        _ => false,
    }
}

#[inline]
fn or(slice: &'static [u8], id: u16, defines: &Vec<Column>, v1: &Filter, v2: &Filter) -> bool {
    v1.is_match(id, defines, slice) || v2.is_match(id, defines, slice)
}

#[inline]
fn and(slice: &'static [u8], id: u16, defines: &Vec<Column>, v1: &Filter, v2: &Filter) -> bool {
    v1.is_match(id, defines, slice) && v2.is_match(id, defines, slice)
}

#[inline]
fn op_eq(slice: &'static [u8], id: u16, defines: &Vec<Column>, v1: &Filter, v2: &Filter) -> bool {
    compare_slice_val(slice, id, defines, v1, v2, eq)
}
#[inline]
fn op_gt_eq(
    slice: &'static [u8],
    id: u16,
    defines: &Vec<Column>,
    v1: &Filter,
    v2: &Filter,
) -> bool {
    compare_slice_val(slice, id, defines, v1, v2, gt_eq)
}
#[inline]
fn op_gt(slice: &'static [u8], id: u16, defines: &Vec<Column>, v1: &Filter, v2: &Filter) -> bool {
    compare_slice_val(slice, id, defines, v1, v2, gt)
}
#[inline]
fn op_lt_eq(
    slice: &'static [u8],
    id: u16,
    defines: &Vec<Column>,
    v1: &Filter,
    v2: &Filter,
) -> bool {
    compare_slice_val(slice, id, defines, v1, v2, lt_eq)
}
#[inline]
fn op_lt(slice: &'static [u8], id: u16, defines: &Vec<Column>, v1: &Filter, v2: &Filter) -> bool {
    compare_slice_val(slice, id, defines, v1, v2, lt)
}
#[inline]
fn op_neq(slice: &'static [u8], id: u16, defines: &Vec<Column>, v1: &Filter, v2: &Filter) -> bool {
    compare_slice_val(slice, id, defines, v1, v2, neq)
}

#[inline]
fn compare_slice_val<F>(
    slice: &'static [u8],
    id: u16,
    defines: &Vec<Column>,
    v1: &Filter,
    v2: &Filter,
    f: F,
) -> bool
where
    F: Fn(Element, Element) -> bool,
{
    match (v1, v2) {
        (Filter::Original(expr1), Filter::Original(expr2)) => match (expr1, expr2) {
            (ExprEntity::Field(idx), ExprEntity::Val(v_type)) => {
                compare_with_op(slice, id, defines, *idx, v_type, f)
            }
            (ExprEntity::FieldWithTab(_, field_idx), ExprEntity::Val(v_type)) => {
                compare_with_op(slice, id, defines, *field_idx, v_type, f)
            }
            (ExprEntity::Val(v_type), ExprEntity::Field(idx)) => {
                compare_with_op(slice, id, defines, *idx, v_type, f)
            }
            (ExprEntity::Val(v_type), ExprEntity::FieldWithTab(_, field_idx)) => {
                compare_with_op(slice, id, defines, *field_idx, v_type, f)
            }
            _ => false,
        },
        _ => false,
    }
}

#[inline]
fn compare_with_op<F>(
    slice: &'static [u8],
    id: u16,
    defines: &Vec<Column>,
    idx: u16,
    v_type: &ValType,
    f: F,
) -> bool
where
    F: Fn(Element, Element) -> bool,
{
    let opt_expacted = fetch_expacted(v_type);
    if let Some(expacted) = opt_expacted {
        let val = fetch_val(slice, id, defines, idx);
        compare_val(expacted, val, f)
    } else {
        false
    }
}

#[inline]
fn fetch_expacted(v_type: &ValType) -> Option<Element> {
    match v_type {
        ValType::Int(val) => Some(Element::Long(*val as u64)),
        ValType::Float(val) => Some(Element::Double(*val)),
        _ => None,
    }
}

#[inline]
fn compare_val<F>(expacted: Element, val: Element, f: F) -> bool
where
    F: Fn(Element, Element) -> bool,
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

    fn fetch(&self, id: u16, defines: &Vec<Column>, slice: &'static [u8]) -> [u8; 512] {
        let mut result = [0u8; 512];
        let target = result.as_mut_slice();
        let mut val;
        let mut offset = 0usize;
        let len = self.executors.len();
        for i in 0..len {
            val = self.executors[i].fetch(id, defines, slice);
            val.copy_to_target(&mut target[offset..offset + 8]);
            offset += 8;
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
    fn is_match(&self, id: u16, defines: &Vec<Column>, slice: &'static [u8]) -> bool {
        match self {
            Filter::Empty => true,
            Filter::Original(_) => false,
            Filter::Mixed(filters) => self.filter_slice(id, defines, filters, slice),
        }
    }

    fn filter_slice(
        &self,
        id: u16,
        defines: &Vec<Column>,
        filters: &[Filter],
        slice: &'static [u8],
    ) -> bool {
        let v1 = &filters[0];
        let v2 = &filters[1];
        let op = &filters[2];
        match op {
            Filter::Empty => true,
            Filter::Original(expr) => self.filter_slice_by_expr(id, defines, expr, v1, v2, slice),
            Filter::Mixed(_) => false,
        }
    }

    fn filter_slice_by_expr(
        &self,
        id: u16,
        defines: &Vec<Column>,
        expr: &ExprEntity,
        v1: &Filter,
        v2: &Filter,
        slice: &'static [u8],
    ) -> bool {
        match expr {
            ExprEntity::Op(op_type) => match op_type {
                OpType::Or => op_or(slice, id, defines, v1, v2),
                OpType::And => op_and(slice, id, defines, v1, v2),
                OpType::Eq => op_eq(slice, id, defines, v1, v2),
                OpType::GtEq => op_gt_eq(slice, id, defines, v1, v2),
                OpType::Gt => op_gt(slice, id, defines, v1, v2),
                OpType::LtEq => op_lt_eq(slice, id, defines, v1, v2),
                OpType::Lt => op_lt(slice, id, defines, v1, v2),
                OpType::Neq => op_neq(slice, id, defines, v1, v2),
            },
            _ => false,
        }
    }
}
