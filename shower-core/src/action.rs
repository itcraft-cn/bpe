use std::str::FromStr;

use strum_macros::EnumString;

use crate::sql::{ExprEntity, ParsedSql};

pub(crate) fn gen_action(parsed_sql: &ParsedSql) -> Vec<Action> {
    let mut actions = vec![];
    let tab_id_array = parsed_sql.tables();
    let filter = create_filter(parsed_sql.filters());
    log::info!("filter: {:?}", filter);
    actions.push(Action::FilterAction(filter));
    let executors = create_executor(tab_id_array.clone(), parsed_sql.fields());
    log::info!("executors: {:?}", executors);
    actions.push(Action::ExecuteAction(executors));
    actions
}

fn create_filter(entities: Vec<ExprEntity>) -> Filter {
    let mut filters = vec![];
    for entity in entities {
        filters.insert(0, Filter::Original(entity));
    }
    loop {
        let len = filters.len();
        if filters.is_empty() {
            break;
        } else if len == 1 {
            log::info!("Filter created");
            break;
        } else if len >= 3 {
            let v1 = filters.remove(0);
            let v2 = filters.remove(0);
            let op = filters.remove(0);
            let new_filter = match (v1.clone(), v2.clone(), op.clone()) {
                (Filter::Original(_), Filter::Original(_), Filter::Original(_)) => {
                    log::info!("0: {:?}, {:?}, {:?}", v1.clone(), v2.clone(), op.clone());
                    Filter::Mixed(vec![v1.clone(), v2.clone(), op.clone()])
                }
                (Filter::Original(_), Filter::Mixed(_), Filter::Original(_)) => {
                    log::info!("1: {:?}, {:?}, {:?}", v1.clone(), v2.clone(), op.clone());
                    Filter::Mixed(vec![v1.clone(), v2.clone(), op.clone()])
                }
                (Filter::Mixed(_), Filter::Original(_), Filter::Original(_)) => {
                    log::info!("2: {:?}, {:?}, {:?}", v1.clone(), v2.clone(), op.clone());
                    Filter::Mixed(vec![v1.clone(), v2.clone(), op.clone()])
                }
                (Filter::Mixed(_), Filter::Mixed(_), Filter::Original(_)) => {
                    log::info!("3: {:?}, {:?}, {:?}", v1.clone(), v2.clone(), op.clone());
                    Filter::Mixed(vec![v1.clone(), v2.clone(), op.clone()])
                }
                _ => {
                    log::warn!(
                        "cannot hit this case: {:?}",
                        (v1.clone(), v2.clone(), op.clone())
                    );
                    continue;
                }
            };
            filters.insert(0, new_filter);
        } else {
            log::warn!("cannot create filter, less than 3");
        }
    }
    filters.get(0).cloned().unwrap()
}

fn create_executor(tab_id_array: Vec<u16>, entities: Vec<ExprEntity>) -> Vec<Executor> {
    let mut executors = vec![];
    for entity in entities {
        let executor = conv_as_executor(entity, &tab_id_array);
        executors.push(executor);
    }
    executors
}

fn conv_as_executor(entity: ExprEntity, tab_id_array: &Vec<u16>) -> Executor {
    let fetcher = match entity {
        ExprEntity::Field(field_id) => {
            Executor::Fetch(tab_id_array.get(0).cloned().unwrap(), field_id)
        }
        ExprEntity::FieldWithTab(tab_id, field_id) => Executor::Fetch(tab_id, field_id),
        ExprEntity::Function(func_name, args) => {
            if func_name.starts_with("_") {
                let mut real_func_name = func_name.clone();
                real_func_name.remove(0);
                let opt_func = Func::from_str(real_func_name.as_str());
                if let Ok(func) = opt_func {
                    let args_fetchers = args
                        .iter()
                        .map(|arg| conv_as_executor(arg.clone(), tab_id_array))
                        .collect::<Vec<Executor>>();
                    Executor::Compute(func, args_fetchers)
                } else {
                    log::warn!("unknown func: {:?}", func_name);
                    panic!("unknown func");
                }
            } else {
                log::warn!("unknown func: {:?}", func_name);
                panic!("unknown func");
            }
        }
        _ => {
            log::warn!("cannot hit this case: {:?}", entity);
            panic!("cannot hit this case");
        }
    };
    fetcher
}

#[derive(Debug, Clone)]
pub(crate) enum Action {
    FilterAction(Filter),
    ExecuteAction(Vec<Executor>),
}
#[derive(Debug, Clone)]
pub(crate) enum Filter {
    Original(ExprEntity),
    Mixed(Vec<Filter>),
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
