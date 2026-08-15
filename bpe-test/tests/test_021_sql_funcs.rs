use bpe::{
    def_aggregate, def_incoming, def_mapper, def_mapper_bind_aggregate, def_stream, new_data,
    start, stop, Column, U8Bytes,
};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

/// The engine is a single-threaded process-level singleton, so all tests in this
/// file must run serially.
static ENGINE_LOCK: Mutex<()> = Mutex::new(());

fn lock_engine() -> std::sync::MutexGuard<'static, ()> {
    ENGINE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Sends one record with columns a(long), b(long), x(double) at offsets 0, 8, 16.
fn send_abx(id: u16, a: i64, b: i64, x: f64) -> bool {
    let mut bytes = [0_u8; 512];
    unsafe {
        *(bytes.as_mut_ptr() as *mut i64) = a;
        *(bytes.as_mut_ptr().add(8) as *mut i64) = b;
        *(bytes.as_mut_ptr().add(16) as *mut f64) = x;
    }
    new_data(&U8Bytes::new_from_vec(id, 512, Vec::from(bytes)))
}

const fn i64v(bits: u64) -> i64 {
    bits as i64
}

/// Scalar functions in SELECT fields (interpreter path).
/// Record: a=9, b=2, x=-3.7
#[test]
fn test_scalar_select() {
    let _g = lock_engine();
    start();
    let in_id = def_incoming(
        "fn_demo1",
        vec![
            Column::new_long("a"),
            Column::new_long("b"),
            Column::new_double("x"),
        ],
    )
    .unwrap();
    let got = Arc::new(AtomicU64::new(0));
    let g2 = got.clone();
    def_mapper(
        r#"SELECT _abs(fn_demo1.a), _pow(fn_demo1.a, 2), _sqrt(fn_demo1.a), _sign(fn_demo1.a),
                  _to_double(fn_demo1.a), _greatest(fn_demo1.a, fn_demo1.b), _least(fn_demo1.a, fn_demo1.b),
                  _to_long(fn_demo1.x), _ceil(fn_demo1.x), _floor(fn_demo1.x), _round(fn_demo1.x), _trunc(fn_demo1.x)
           FROM fn_demo1 WHERE fn_demo1.a >= 1 LIMIT 10"#,
        move |p| {
            assert_eq!(p.size(), 1);
            let ptr = p.u8_ptr();
            // i64 fields at 0,8,16,24 ; f64 fields at 32,40,48 ; i64 56 ; f64 64,72,80,88
            let abs_v: i64 = unsafe { *(ptr as *const i64) };
            let pow_v: f64 = unsafe { *(ptr.add(8) as *const f64) };
            let sqrt_v: f64 = unsafe { *(ptr.add(16) as *const f64) };
            let sign_v: i64 = unsafe { *(ptr.add(24) as *const i64) };
            let tod_v: f64 = unsafe { *(ptr.add(32) as *const f64) };
            let grt_v: i64 = unsafe { *(ptr.add(40) as *const i64) };
            let lst_v: i64 = unsafe { *(ptr.add(48) as *const i64) };
            let tol_v: i64 = unsafe { *(ptr.add(56) as *const i64) };
            let ceil_v: f64 = unsafe { *(ptr.add(64) as *const f64) };
            let floor_v: f64 = unsafe { *(ptr.add(72) as *const f64) };
            let round_v: f64 = unsafe { *(ptr.add(80) as *const f64) };
            let trunc_v: f64 = unsafe { *(ptr.add(88) as *const f64) };
            assert_eq!(abs_v, 9, "abs(9)");
            assert!((pow_v - 81.0).abs() < 1e-9, "pow(9,2)={pow_v}");
            assert!((sqrt_v - 3.0).abs() < 1e-9, "sqrt(9)={sqrt_v}");
            assert_eq!(sign_v, 1, "sign(9)");
            assert!((tod_v - 9.0).abs() < 1e-9, "to_double(9)={tod_v}");
            assert_eq!(grt_v, 9, "greatest(9,2)");
            assert_eq!(lst_v, 2, "least(9,2)");
            assert_eq!(tol_v, -3, "to_long(-3.7)={tol_v}");
            assert!((ceil_v - -3.0).abs() < 1e-9, "ceil(-3.7)={ceil_v}");
            assert!((floor_v - -4.0).abs() < 1e-9, "floor(-3.7)={floor_v}");
            assert!((round_v - -4.0).abs() < 1e-9, "round(-3.7)={round_v}");
            assert!((trunc_v - -3.0).abs() < 1e-9, "trunc(-3.7)={trunc_v}");
            g2.store(1, Ordering::SeqCst);
        },
    )
    .unwrap();
    assert!(send_abx(in_id, 9, 2, -3.7));
    assert_eq!(got.load(Ordering::SeqCst), 1, "scalar select callback must fire");
    stop();
}

/// Scalar functions in WHERE clauses (JIT path).
#[test]
fn test_where_functions() {
    let _g = lock_engine();
    start();
    let in_id = def_incoming(
        "fn_demo2",
        vec![Column::new_long("a"), Column::new_long("b")],
    )
    .unwrap();
    // _abs(a) > 5
    let s_abs = Arc::new(AtomicU64::new(0));
    let a1 = s_abs.clone();
    def_mapper(
        "SELECT fn_demo2.a FROM fn_demo2 WHERE _abs(fn_demo2.a) > 5 LIMIT 10",
        move |p| {
            a1.store(p.size() as u64, Ordering::SeqCst);
        },
    )
    .unwrap();
    // _pow(a, 2) > 40
    let s_pow = Arc::new(AtomicU64::new(0));
    let a2 = s_pow.clone();
    def_mapper(
        "SELECT fn_demo2.a FROM fn_demo2 WHERE _pow(fn_demo2.a, 2) > 40 LIMIT 10",
        move |p| {
            a2.store(p.size() as u64, Ordering::SeqCst);
        },
    )
    .unwrap();
    // nested: _greatest(_abs(a), b) > 5
    let s_grt = Arc::new(AtomicU64::new(0));
    let a3 = s_grt.clone();
    def_mapper(
        "SELECT fn_demo2.a FROM fn_demo2 WHERE _greatest(_abs(fn_demo2.a), fn_demo2.b) > 5 LIMIT 10",
        move |p| {
            a3.store(p.size() as u64, Ordering::SeqCst);
        },
    )
    .unwrap();
    // a=3: abs=3 (no), pow=9 (no), greatest(max(3,b)) -> b=6 -> yes
    assert!(send_abx(in_id, 3, 6, 0.0));
    assert_eq!(s_abs.load(Ordering::SeqCst), 0, "_abs(3)>5 must not match");
    assert_eq!(s_pow.load(Ordering::SeqCst), 0, "_pow(3,2)>40 must not match");
    assert_eq!(s_grt.load(Ordering::SeqCst), 1, "greatest(abs(3),6)>5 must match");
    // a=7: all match
    assert!(send_abx(in_id, 7, 0, 0.0));
    assert_eq!(s_abs.load(Ordering::SeqCst), 1, "_abs(7)>5 must match");
    assert_eq!(s_pow.load(Ordering::SeqCst), 1, "_pow(7,2)>40 must match");
    stop();
}

/// Aggregate functions _stddev / _variance / _stddev_samp / _var_samp.
/// Input a=1,2,3: population var=2/3, stddev=sqrt(2/3); sample var=1, stddev=1.
#[test]
fn test_aggregate_stddev() {
    let _g = lock_engine();
    start();
    let in_id = def_incoming("fn_demo3", vec![Column::new_long("a")]).unwrap();
    def_stream("fn_stream3", vec![Column::new_long("a")]).unwrap();
    let got = Arc::new(AtomicU64::new(0));
    let calls = Arc::new(AtomicU64::new(0));
    let g2 = got.clone();
    let c2 = calls.clone();
    let agg_id = def_aggregate(
        "select _stddev(fn_stream3.a), _variance(fn_stream3.a), _stddev_samp(fn_stream3.a), _var_samp(fn_stream3.a) from fn_stream3",
        move |p| {
            let n = c2.fetch_add(1, Ordering::SeqCst) + 1;
            let ptr = p.u8_ptr();
            let sd: f64 = unsafe { *(ptr as *const f64) };
            let vr: f64 = unsafe { *(ptr.add(8) as *const f64) };
            let sds: f64 = unsafe { *(ptr.add(16) as *const f64) };
            let vrs: f64 = unsafe { *(ptr.add(24) as *const f64) };
            if n == 3 {
                // window = [1,2,3] -> population var=2/3, stddev=sqrt(2/3); sample var=1, stddev=1
                let exp_sd = (2.0f64 / 3.0f64).sqrt();
                assert!((sd - exp_sd).abs() < 1e-9, "stddev={sd}, expect {exp_sd}");
                assert!((vr - 2.0 / 3.0).abs() < 1e-9, "variance={vr}");
                assert!((sds - 1.0).abs() < 1e-9, "stddev_samp={sds}");
                assert!((vrs - 1.0).abs() < 1e-9, "var_samp={vrs}");
                g2.store(1, Ordering::SeqCst);
            }
        },
    )
    .unwrap();
    def_mapper_bind_aggregate(
        "SELECT fn_demo3.a FROM fn_demo3 WHERE fn_demo3.a >= 1 LIMIT 10",
        agg_id,
    )
    .unwrap();
    for v in [1i64, 2, 3] {
        let mut bytes = [0_u8; 512];
        unsafe { *(bytes.as_mut_ptr() as *mut i64) = v; }
        new_data(&U8Bytes::new_from_vec(in_id, 512, Vec::from(bytes)));
    }
    assert_eq!(got.load(Ordering::SeqCst), 1, "stddev aggregate must be verified");
    stop();
}

/// Aggregate _stddev on Double columns and the mixed-type count fix.
#[test]
fn test_aggregate_stddev_double_and_count() {
    let _g = lock_engine();
    start();
    let in_id = def_incoming("fn_demo4", vec![Column::new_double("x")]).unwrap();
    def_stream("fn_stream4", vec![Column::new_double("x")]).unwrap();
    let got = Arc::new(AtomicU64::new(0));
    let calls = Arc::new(AtomicU64::new(0));
    let g2 = got.clone();
    let c2 = calls.clone();
    let agg_id = def_aggregate(
        "select _stddev(fn_stream4.x), _count(fn_stream4.x) from fn_stream4",
        move |p| {
            let n = c2.fetch_add(1, Ordering::SeqCst) + 1;
            let ptr = p.u8_ptr();
            let sd: f64 = unsafe { *(ptr as *const f64) };
            let cnt: i64 = unsafe { *(ptr.add(8) as *const i64) };
            if n == 2 {
                // window = [1.0, 3.0] -> population stddev = 1
                assert!((sd - 1.0).abs() < 1e-9, "stddev of [1,3] = {sd}");
                assert_eq!(cnt, 2, "count on double column");
                g2.store(1, Ordering::SeqCst);
            }
        },
    )
    .unwrap();
    def_mapper_bind_aggregate(
        "SELECT fn_demo4.x FROM fn_demo4 WHERE fn_demo4.x >= 1 LIMIT 10",
        agg_id,
    )
    .unwrap();
    for v in [1.0f64, 3.0] {
        let mut bytes = [0_u8; 512];
        unsafe { *(bytes.as_mut_ptr() as *mut f64) = v; }
        new_data(&U8Bytes::new_from_vec(in_id, 512, Vec::from(bytes)));
    }
    assert_eq!(got.load(Ordering::SeqCst), 1, "stddev on double must be verified");
    stop();
}

/// Helper referenced to keep clippy quiet about the const fn (documentation value).
const _: i64 = i64v(0);
