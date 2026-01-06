use crate::{cfg, consts::BPE_ENV_HOME_KEY, logger};
use std::env;

pub(crate) fn test_init() {
    env::set_var(BPE_ENV_HOME_KEY, "/home/helly/code/rust/bpe");
    cfg::load_config();
    logger::init_logger();
}
