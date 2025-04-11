use crate::{cfg, consts::SHOWER_ENV_HOME_KEY, logger};
use std::env;

pub(crate) fn test_init() {
    env::set_var(SHOWER_ENV_HOME_KEY, "/home/helly/code/rust/bambootube");
    cfg::load_config();
    logger::init_logger();
}
