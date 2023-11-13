use shower::{def_mapper, start, stop};
use std::env;

const SQL: &str = r#"
    SELECT _1.__1, _1.__2, _1.__3, _sub(_add(_1.__4, _1.__4), _1.__5)
    FROM _1
    WHERE (_1.__1 = '1' AND _1.__2 = '2') OR (_1.__1 = '3' AND _1.__2 = '4')
    LIMIT 10
    "#;

#[test]
fn test_sql_parse() {
    env::set_var("SHOWER_HOME", "/home/helly/code/rust/shower");
    start();
    def_mapper(SQL, |_vec| {});
    stop();
}
