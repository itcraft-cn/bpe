#[cfg(test)]
mod tests {
    use crate::{consts::SHOWER_ENV_HOME_KEY, new_data, start, stop, QuoteData};
    use log::*;
    use std::{env, thread, time::Duration};

    #[test]
    fn test_new_proc() {
        env::set_var(SHOWER_ENV_HOME_KEY, "/home/helly/code/rust/shower");
        start();
        let mut vec = vec![];
        for idx in 0..4 {
            let rs = thread::Builder::new()
                .name(format!("caller-{}", idx))
                .spawn(move || {
                    gen_new_data();
                });
            if rs.is_ok() {
                vec.push(rs.unwrap());
            }
        }
        thread::sleep(Duration::from_secs(30));
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

    fn gen_new_data() {
        info!("thread:{} started", thread::current().name().unwrap());
        for i in 0..10000000 {
            let v = i as u64;
            let ret = new_data(QuoteData::new(
                0, v as u128, v as u128, v as u128, v as u128, v,
            ));
            if ret {
                debug!("send success");
            } else {
                warn!("send failed");
            }
        }
    }
}
