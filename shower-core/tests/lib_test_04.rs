mod test_log;

use shower::{def_mapper, start, stop};

#[test]
fn test_new_proc() {
    let test_sql_vec: Vec<&str> = vec![
        r#"SELECT demo.a FROM demo LIMIT 1"#,
        r#"SELECT _add(demo.a, demo.b) FROM demo LIMIT 1"#,
        r#"SELECT _sub(demo.a, demo.b) FROM demo LIMIT 1"#,
        r#"SELECT _mul(demo.a, demo.b) FROM demo LIMIT 1"#,
        r#"SELECT _div(demo.a, demo.b) FROM demo LIMIT 1"#,
        r#"SELECT _mod(demo.a, demo.b) FROM demo LIMIT 1"#,
    ];
    test_log::setup_shower_home();
    start();
    for sql in test_sql_vec {
        def_mapper(sql, |_vec| {});
    }
    stop();
}
