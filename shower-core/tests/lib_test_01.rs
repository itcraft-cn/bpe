mod test_log;

use shower::{def_mapper, start, stop};

const SQL: &str = r#"
    SELECT demo.a, demo.b, demo.c, _sub(_add(demo.d, demo.d), demo.e)
    FROM demo
    WHERE (demo.a = 1 AND demo.b = 2) OR (demo.a = 3 AND demo.b = 4)
    LIMIT 10
    "#;

#[test]
fn test_sql_parse() {
    test_log::setup_shower_home();
    start();
    def_mapper(SQL, |_vec| {});
    stop();
}
