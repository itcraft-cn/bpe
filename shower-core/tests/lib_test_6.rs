use shower::{def_incoming, start, stop, Column};
use std::env;

#[test]
fn test_new_proc() {
    env::set_var("SHOWER_HOME", "/home/helly/code/rust/shower");
    start();
    let id = def_incoming(vec![
        Column::new_long(),
        Column::new_double(),
        Column::new_long(),
        Column::new_double(),
        Column::new_string(240),
    ]);
    assert_eq!(1, id);
    log::info!("new id: {}", id);
    stop();
}
