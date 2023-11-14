use shower::{def_incoming, start, stop, Column};
use std::env;

#[test]
fn test_new_proc() {
    env::set_var("SHOWER_HOME", "/home/helly/code/rust/shower");
    start();
    if let Some(id) = def_incoming(
        "demo",
        vec![
            Column::new_long("a"),
            Column::new_double("b"),
            Column::new_long("c"),
            Column::new_double("d"),
            Column::new_string("e", 240),
        ],
    ) {
        assert_eq!(1, id);
        log::info!("new id: {}", id);
    }
    stop();
}
