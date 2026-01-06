mod test_log;

use bpe::{def_incoming, start, stop, Column};

#[test]
fn test_new_proc() {
    test_log::setup_bpe_home();
    start();
    if let Some(id) = def_incoming(
        "demo",
        vec![
            Column::new_long("a"),
            Column::new_double("b"),
            Column::new_long("c"),
            Column::new_double("d"),
            Column::new_long("e"),
        ],
    ) {
        assert_eq!(1, id);
        log::info!("new id: {id}");
    }
    stop();
}
