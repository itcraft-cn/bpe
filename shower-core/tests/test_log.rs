use std::sync::Once;

/// 日志初始化，写入 `stdout`，并写入临时文件夹下 `shower.log`
pub fn init_logger() {
    static INIT: Once = Once::new();
    INIT.call_once(init_log4rs);
    log::info!("booting up");
}

fn init_log4rs() {
    let _ = log4rs::init_file("./cfg/log4rs.dev.yaml", Default::default());
}
