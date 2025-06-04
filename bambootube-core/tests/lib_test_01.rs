mod test_log;

use bambootube::{def_incoming, def_mapper, start, stop, Column};

const SQL1: &str = r#"
    SELECT demo1.a, demo1.b, demo1.c, _sub(_add(demo1.d, demo1.d), demo1.e)
    FROM demo1
    WHERE (demo1.a = 1 AND demo1.b = 2) OR (demo1.a = 3 AND demo1.b = 4)
    LIMIT 10
    "#;
const SQL2: &str = r#"
    SELECT demo2.a, demo2.b, demo2.c, _sub(_add(demo2.d, demo2.d), demo2.e)
    FROM demo2
    WHERE (a = 1 AND b = 2) OR (a = 3 AND b = 4)
    LIMIT 10
    "#;

#[test]
fn test_sql_parse() {
    test_log::setup_bambootube_home();
    test_log::init_logger();
    start();
    call_def_incoming("demo1", 1);
    call_def_incoming("demo2", 2);
    def_mapper(SQL1, |_ptr, _size| {});
    def_mapper(SQL2, |_ptr, _size| {});
    stop();
}

fn call_def_incoming(name: &str, expected_id: u16) {
    if let Some(id) = def_incoming(
        name,
        vec![
            Column::new_long("a"),
            Column::new_double("b"),
            Column::new_long("c"),
            Column::new_double("d"),
            Column::new_long("e"),
        ],
    ) {
        assert_eq!(expected_id, id);
        log::info!("new id: {}", id);
    }
}
