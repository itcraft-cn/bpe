use shower::{def_action, start, stop};
use std::env;

#[test]
fn test_sql_parse() {
    env::set_var("SHOWER_HOME", "/home/helly/code/rust/shower");
    start();
    def_action(
        r#"SELECT _1.__1, _2.__2, _1.__3, _sub(_add(_1.__4, _2.__4), _2.__5) FROM _1, _2 WHERE _1.__1 = '1' AND _2.__2 = '2' AND _1.__3 = _2.__3 LIMIT 10"#,
    );
    stop();
}
