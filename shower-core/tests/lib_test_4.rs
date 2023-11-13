use shower::{def_mapper, start, stop};
use std::env;

#[test]
fn test_new_proc() {
    let test_sql_vec: Vec<&str> = vec![
        r#"SELECT _1.__1 FROM _1 LIMIT 1"#,
        r#"SELECT _add(_1.__1, _1.__2) FROM _1 LIMIT 1"#,
        r#"SELECT _sub(_1.__1, _1.__2) FROM _1 LIMIT 1"#,
        r#"SELECT _mul(_1.__1, _1.__2) FROM _1 LIMIT 1"#,
        r#"SELECT _div(_1.__1, _1.__2) FROM _1 LIMIT 1"#,
        r#"SELECT _mod(_1.__1, _1.__2) FROM _1 LIMIT 1"#,
    ];
    env::set_var("SHOWER_HOME", "/home/helly/code/rust/shower");
    start();
    for sql in test_sql_vec {
        def_mapper(sql);
    }
    stop();
}
