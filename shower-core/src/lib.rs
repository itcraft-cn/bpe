#[macro_use]
pub(crate) mod macros;

pub(crate) mod cfg;
pub(crate) mod consts;
pub(crate) mod core;
pub(crate) mod logger;

pub fn start() {
    core::start();
}

pub fn stop() {
    core::stop();
}

pub fn new_data(data: [u8; 8192]) {
    core::new_data(data);
}

#[cfg(test)]
mod tests {
    use crate::{consts::SHOWER_ENV_HOME_KEY, new_data, start, stop};
    use std::{env, thread, time::Duration};

    #[test]
    fn test() {
        env::set_var(SHOWER_ENV_HOME_KEY, "/home/helly/code/rust/shower");
        let data = [0u8; 8192];
        start();
        let mut vec = vec![];
        for _ in 0..4 {
            vec.push(thread::spawn(move || {
                for _ in 0..10000000 {
                    new_data(data.clone());
                }
            }));
        }
        loop {
            if vec.iter().all(|t| t.is_finished()) {
                break;
            } else {
                const DURATION: Duration = Duration::from_millis(100);
                thread::sleep(DURATION);
            }
        }
        stop();
    }
}
