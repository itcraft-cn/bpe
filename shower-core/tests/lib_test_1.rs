use shower::{def_action, start, stop};
use std::env;

#[test]
fn test_sql_parse() {
    env::set_var("SHOWER_HOME", "/home/helly/code/rust/shower");
    start();
    def_action(r#"SELECT __1 FROM _1 WHERE _1.__1 = 1"#);
    stop();
}
