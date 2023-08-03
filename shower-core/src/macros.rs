#[macro_export]
macro_rules! if_match_log {
    ($r:expr, $desc:expr) => {
        if $r.is_err() {
            log::warn!("hit error: {:?}, desc: {:?}", $r.err().unwrap(), $desc);
        }
    };
}

#[macro_export]
macro_rules! if_match_panic {
    ($r:expr, $desc:expr) => {
        if $r.is_err() {
            panic!($desc);
        }
    };
}

#[macro_export]
macro_rules! if_match_log_and_panic {
    ($r:expr, $desc:expr) => {
        if $r.is_err() {
            log::warn!("hit error: {:?}, desc: {:?}", $r.err().unwrap(), $desc);
            panic!($desc);
        }
    };
}

#[macro_export]
macro_rules! warn_and_panic {
    ($template:expr, $arg:expr) => {
        log::warn!($template, $arg);
        panic!("hit error");
    };
}
