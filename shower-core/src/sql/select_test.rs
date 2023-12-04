use super::{base, select};
use crate::{cfg, data, id, logger};

#[test]
fn test() {
    cfg::load_config();
    logger::init_dev_logger();
    let sql_vec = vec![
        r#"
        SELECT demo.a, demo.b, demo.c, _sub(_add(demo.d, demo.d), demo.e)
        FROM demo
        WHERE (demo.a = 1 AND demo.b = 2) OR (demo.a = 3 AND demo.b = 4)
        LIMIT 10
        "#,
        r#"
        select _maxl(stream.a), _minl(stream.a), _suml(stream.a),
            _maxd(stream.a), _mind(stream.a), _sumd(stream.a),
            _avg(stream.a), _count(stream.a)
        from stream
        "#,
        r#"
        SELECT demo.a, demo.b, demo.c, _sub(_add(demo.d, demo.d), demo.e)
        FROM demo
        WHERE (demo.a = 1 AND demo.b = 2) OR (demo.a = 3 AND demo.b = 4)
        INTERVAL 10
        "#,
    ];
    id::init_walker();
    data::init_record_store();
    data::Record::insert_record(
        "demo",
        data::RecordType::Incoming,
        vec![
            data::Column::new_long("a"),
            data::Column::new_long("b"),
            data::Column::new_long("c"),
            data::Column::new_long("d"),
            data::Column::new_long("e"),
            data::Column::new_long("f"),
            data::Column::new_long("g"),
            data::Column::new_long("h"),
            data::Column::new_string("i", 448),
        ],
    );
    data::Record::insert_record(
        "stream",
        data::RecordType::Stream,
        vec![
            data::Column::new_long("a"),
            data::Column::new_long("b"),
            data::Column::new_long("c"),
            data::Column::new_double("d"),
            data::Column::new_double("e"),
            data::Column::new_double("f"),
            data::Column::new_double("g"),
            data::Column::new_long("h"),
        ],
    );
    for sql in sql_vec {
        if let Some(parsed_sql) = select::parse_select(sql, &base::parse_options()) {
            log::info!("{:?}", parsed_sql);
        } else {
            log::warn!("{:?}", sql);
        }
    }
}
