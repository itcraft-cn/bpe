#[macro_use]
pub(crate) mod macros;

pub(crate) mod cfg;
pub(crate) mod consts;
pub(crate) mod core;
pub(crate) mod data;
pub(crate) mod logger;
pub(crate) mod store;

#[cfg(test)]
mod lib_test;

pub use crate::core::def_action;
pub use crate::core::new_data;
pub use crate::core::start;
pub use crate::core::stop;
pub use crate::data::Bar;
pub use crate::data::BarType;
pub use crate::data::DeepTick;
pub use crate::data::Tick;
