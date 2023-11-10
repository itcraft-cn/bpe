use shower::{def_record, start, stop, Column};
use std::env;

#[test]
fn test_new_proc() {
    env::set_var("SHOWER_HOME", "/home/helly/code/rust/shower");
    start();
    let id = def_record(vec![
        Column::new_long(),
        Column::new_double(),
        Column::new_long(),
        Column::new_double(),
        Column::new_string(240),
    ]);
    assert_eq!(0, id);
    log::info!("new id: {}", id);
    stop();
}
