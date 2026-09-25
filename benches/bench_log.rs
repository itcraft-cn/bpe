#![allow(dead_code)]

use log::LevelFilter;
use log4rs::{
    append::{
        console::ConsoleAppender,
        rolling_file::{
            policy::compound::{
                roll::fixed_window::FixedWindowRoller, trigger::size::SizeTrigger, CompoundPolicy,
            },
            RollingFileAppender,
        },
    },
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
    Config,
};
use std::{env, path::PathBuf, sync::Once};

const PATTERN_STDOUT: &str = "[{d}][{h({l})}][{I}|{h({T})}][{t}\\(L{L}\\)] {m}{n}";
const PATTERN_FILE: &str = "[{d}][{l}][{I}|{T}][{t}\\(L{L}\\)] {m}{n}";

const LOG_NAME: &str = "bpe.log";

const ROLLING_FILE_PATTERN: &str = "bpe.{}.log";
const ROLLING_FILE_MAX_COUNT: u32 = 20;
const ROLLING_FILE_SIZE_200M: u64 = 200 * 1024 * 1024;

pub(crate) fn init_logger() {
    static INIT: Once = Once::new();
    INIT.call_once(init_log4rs);
}

fn init_log4rs() {
    let console_appender = init_console();
    let file_appender = init_file();
    let root = Root::builder()
        .appender("stdout")
        .appender("file")
        .build(LevelFilter::Info);
    if let Ok(config) = Config::builder()
        .appender(console_appender)
        .appender(file_appender)
        .build(root)
    {
        if let Ok(_handle) = log4rs::init_config(config) {
            log::info!("Log4rs initialized.");
        } else {
            eprintln!("failed to initialize log4rs");
        }
    } else {
        eprintln!("failed to initialize log4rs");
    }
}

fn init_console() -> Appender {
    let encoder_stdout = Box::new(PatternEncoder::new(PATTERN_STDOUT));
    let console_appender = Box::new(ConsoleAppender::builder().encoder(encoder_stdout).build());
    Appender::builder().build("stdout", console_appender)
}

fn init_file() -> Appender {
    let log_path = fetch_log_path();
    let encoder_file = Box::new(PatternEncoder::new(PATTERN_FILE));
    let roller = FixedWindowRoller::builder()
        .base(1)
        .build(ROLLING_FILE_PATTERN, ROLLING_FILE_MAX_COUNT)
        .unwrap();
    let policy = Box::new(CompoundPolicy::new(
        Box::new(SizeTrigger::new(ROLLING_FILE_SIZE_200M)),
        Box::new(roller),
    ));
    let file_appender = Box::new(
        RollingFileAppender::builder()
            .encoder(encoder_file)
            .build(log_path, policy)
            .unwrap(),
    );
    Appender::builder().build("file", file_appender)
}

fn fetch_log_path() -> PathBuf {
    let current_dir = env::current_dir().unwrap();
    current_dir.join(LOG_NAME)
}

pub(crate) fn setup_bpe_home() {
    env::set_var("BPE_HOME", env::current_dir().unwrap());
}
