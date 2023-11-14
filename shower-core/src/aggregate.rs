use crate::{aux::SimpleU16Map, func::FnHolder, sql::base::parse_options};
use sql_parse::ParseOptions;

static mut AGGREGATE_MAP: Option<SimpleU16Map<WrappedAggregate>> = None;
static mut PARSE_OPTIONS: Option<ParseOptions> = None;

pub(crate) fn init_aggregate_store() {
    unsafe {
        PARSE_OPTIONS = Some(parse_options());
        AGGREGATE_MAP = Some(SimpleU16Map::new());
    }
}

pub(crate) fn define_aggregate(_sql: &str, _func_holder: FnHolder) -> bool {
    let map = unsafe { AGGREGATE_MAP.as_mut().unwrap() };
    map.insert(1, WrappedAggregate::new(Aggregate {}, _func_holder));
    true
}

pub(crate) fn search_aggregate<'a>(id: u16) -> Option<&'a WrappedAggregate> {
    let map = unsafe { AGGREGATE_MAP.as_ref().unwrap() };
    map.get(id)
}

pub(crate) fn call_aggregate<'a>(_id: u16, _wrapped: &WrappedAggregate, _data: Vec<[u8; 512]>) {
    match &_wrapped._fn_holder {
        FnHolder::Func(f) => f(_data),
        FnHolder::FfiFunc(f) => f.callback(_data),
        FnHolder::Lambda(f) => f(_data),
    }
}

pub(crate) struct Aggregate {}

pub(crate) struct WrappedAggregate {
    _aggregate: Aggregate,
    _fn_holder: FnHolder,
}
impl WrappedAggregate {
    fn new(_aggregate: Aggregate, _fn_holder: FnHolder) -> Self {
        WrappedAggregate {
            _aggregate,
            _fn_holder,
        }
    }
}
