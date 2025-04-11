mod test_log;

use bambootube::{def_mapper, new_data, start, stop, U8Bytes};

const SQL: &str = r#"
    SELECT demo.a FROM demo LIMIT 1
    "#;

#[test]
fn test_new_proc() {
    test_log::setup_bambootube_home();
    start();
    def_mapper(SQL, |_vec, _size| {});
    let u8data = U8Bytes::new_from_vec(1, 64, vec![0_u8; 64]);
    new_data(&u8data);
    stop();
}
