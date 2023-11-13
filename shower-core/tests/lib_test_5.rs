use shower::{def_mapper, start, stop, new_data, U8Bytes};
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
    def_mapper(SQL, |_vec| {});
    let u8data = U8Bytes::new_from_vec(1, 64, vec![0u8; 64]);
    new_data(&u8data);
    stop();
}
