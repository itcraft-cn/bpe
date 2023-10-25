use shower::{def_action, start, stop};
use std::env;

const SQL: &str = r#"
    SELECT _1.__1
    FROM _1
    LIMIT 1
    "#;

#[test]
fn test_new_proc() {
    env::set_var("SHOWER_HOME", "/home/helly/code/rust/shower");
    start();
    def_action(SQL);
    stop();
}
