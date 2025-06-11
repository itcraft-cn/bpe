use crate::{cfg, consts::BAMBOOTUBE_ENV_HOME_KEY, logger};
use std::env;

pub(crate) fn test_init() {
    env::set_var(BAMBOOTUBE_ENV_HOME_KEY, "/home/helly/code/rust/bbpe");
    cfg::load_config();
    logger::init_logger();
}
