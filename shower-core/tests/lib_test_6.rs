use shower::{def_record, start, stop};
use std::env;

#[test]
fn test_new_proc() {
    env::set_var("SHOWER_HOME", "/home/helly/code/rust/shower");
    start();
    let id = def_record(&[1, 3, 1, 2, 4, 1, 2]);
    assert_eq!(-1, id);
    log::info!("new id: {}", id);
    let id = def_record(&[1, 3, 1, 1, 2, 4, 1, 2]);
    assert_eq!(0, id);
    log::info!("new id: {}", id);
    stop();
}
