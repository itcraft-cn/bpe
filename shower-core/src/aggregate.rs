use crate::{aux::SimpleU16Map, func::FnHolder, sql::base::parse_options};
use sql_parse::ParseOptions;

static mut AGGREGATE_MAP: Option<SimpleU16Map<Vec<(Aggregate, FnHolder)>>> = None;
static mut PARSE_OPTIONS: Option<ParseOptions> = None;

pub(crate) fn init_aggregate_store() {
    unsafe {
        PARSE_OPTIONS = Some(parse_options());
        AGGREGATE_MAP = Some(SimpleU16Map::new());
    }
}

pub(crate) fn define_aggregate(_sql: &str, _func_holder: FnHolder) -> bool {
    true
}

pub(crate) fn search_aggregate<'a>(id: u16) -> Option<&'a Vec<(Aggregate, FnHolder)>> {
    let map = unsafe { AGGREGATE_MAP.as_ref().unwrap() };
    map.get(id)
}

pub(crate) struct Aggregate {}
