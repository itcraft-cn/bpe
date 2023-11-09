use shower::{def_record, start, stop, FieldDef, DOUBLE, LONG};
use std::env;

#[test]
fn test_new_proc() {
    env::set_var("SHOWER_HOME", "/home/helly/code/rust/shower");
    start();
    let id = def_record(vec![
        FieldDef::Num(LONG),
        FieldDef::Num(DOUBLE),
        FieldDef::Num(LONG),
        FieldDef::Num(DOUBLE),
        FieldDef::Str(32),
    ]);
    assert_eq!(0, id);
    log::info!("new id: {}", id);
    stop();
}
