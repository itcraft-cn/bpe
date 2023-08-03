#[macro_use]
pub(crate) mod macros;

pub(crate) mod cfg;
pub(crate) mod consts;
pub(crate) mod core;
pub(crate) mod data;
pub(crate) mod logger;

pub use data::QuoteData;

pub fn start() {
    core::start();
}

pub fn stop() {
    core::stop();
}

pub fn new_data(data: QuoteData) -> bool {
    core::new_data(data)
}
