//! JIT-compiled aggregate kernels.
#![allow(deprecated)] // LLVM 15+ unified pointers; inkwell 0.7 still needs typed ptr helpers
//!
//! Aggregates are interpreted by default (per record, per field: column lookup +
//! type dispatch + function dispatch), which dominates the cost of high-frequency
//! aggregation. `gen_aggregate_func` compiles the whole batch aggregation into a
//! single LLVM function:
//!
//! ```text
//! unsafe extern "C" fn(batch: *const u8, count: usize, step: usize, out: *mut u8)
//! ```
//!
//! - column reads are direct `load` at pre-resolved offsets (no record lookup)
//! - arithmetic/comparison is inlined IR (no function dispatch)
//! - accumulating fields (sum/count/avg/min/max/stddev) share one loop with phis
//! - first/last read the first/last record directly (no loop)
//! - constants are written unconditionally; count == 0 writes initial values
//!
//! Result layout matches the interpreter: dense `col_idx * 8` slots.

use crate::{
    aggregate::Aggregate,
    data::{ColumnType, Record},
    exec::Executor,
    func_enum::SupportFunc,
    jit::base::FuncGenerator,
};
use inkwell::{
    builder::Builder,
    context::Context,
    execution_engine::JitFunction,
    types::{BasicTypeEnum, FloatType, IntType},
    values::{BasicValueEnum, FloatValue, IntValue, PhiValue, PointerValue},
    FloatPredicate, IntPredicate,
};

pub(crate) type AggFunc = unsafe extern "C" fn(*const u8, usize, usize, *mut u8);

/// Field plan: how the JIT kernel evaluates one aggregate field.
enum FieldKind {
    ConstLong(i64),
    ConstDouble(f64),
    /// First/last record value (read directly, no loop).
    First { off: usize, out_is_long: bool },
    Last { off: usize, out_is_long: bool },
    /// Accumulating field; `off` is the input column offset, `input_is_long` its type.
    Acc { kind: AccKind, off: usize, input_is_long: bool },
    /// Incompatible input type / unsupported: write the initial value only
    /// (matches the interpreter, which warns and keeps the init).
    Skipped { out_is_long: bool },
}

#[derive(Clone, Copy)]
enum AccKind {
    SumI,
    SumF,
    Count,
    Avg,
    MaxI,
    MinI,
    MaxF,
    MinF,
    Stddev { sample: bool },
    Var { sample: bool },
}

impl AccKind {
    /// Initial values for the accumulator group.
    fn inits(&self, i64_ty: IntType<'static>, f64_ty: FloatType<'static>) -> Vec<BasicValueEnum<'static>> {
        match self {
            AccKind::SumI | AccKind::Count => vec![i64_ty.const_zero().into()],
            AccKind::MaxI => vec![i64_ty.const_int(i64::MIN as u64, true).into()],
            AccKind::MinI => vec![i64_ty.const_int(i64::MAX as u64, true).into()],
            AccKind::SumF => vec![f64_ty.const_float(0.0).into()],
            AccKind::Avg => vec![f64_ty.const_float(0.0).into(), i64_ty.const_zero().into()],
            AccKind::MaxF => vec![f64_ty.const_float(f64::MIN).into()],
            AccKind::MinF => vec![f64_ty.const_float(f64::MAX).into()],
            AccKind::Stddev { .. } | AccKind::Var { .. } => vec![
                f64_ty.const_float(0.0).into(), // count
                f64_ty.const_float(0.0).into(), // mean
                f64_ty.const_float(0.0).into(), // m2
            ],
        }
    }

    /// Accumulator types per slot (i64 or f64).
    fn acc_types(&self, i64_ty: IntType<'static>, f64_ty: FloatType<'static>) -> Vec<BasicTypeEnum<'static>> {
        match self {
            AccKind::SumI | AccKind::Count | AccKind::MaxI | AccKind::MinI => {
                vec![i64_ty.into()]
            }
            AccKind::SumF | AccKind::MaxF | AccKind::MinF => vec![f64_ty.into()],
            AccKind::Avg => vec![f64_ty.into(), i64_ty.into()],
            AccKind::Stddev { .. } | AccKind::Var { .. } => {
                vec![f64_ty.into(), f64_ty.into(), f64_ty.into()]
            }
        }
    }
}

/// Compiles the aggregate kernel. `input_offsets` maps a stream field id to the
/// byte offset of that field in the input record (raw record layout for windows,
/// mapper SELECT output for bind-aggregates).
pub(crate) fn gen_aggregate_func(
    aggregate: &Aggregate,
    input_offsets: &[usize],
) -> Option<JitFunction<'static, AggFunc>> {
    let stream = Record::get_record(aggregate.stream_id())?;
    let fields: Vec<FieldKind> = aggregate
        .executors()
        .iter()
        .map(|e| plan_field(e, &stream, input_offsets))
        .collect();

    let fg = Box::leak(Box::new(FuncGenerator::new()));
    let ctx: &Context = fg.context;
    let i64_ty = ctx.i64_type();
    let f64_ty = ctx.f64_type();
    let void_ty = ctx.void_type();
    let i8_ty = ctx.i8_type();
    let i8_ptr = i8_ty.ptr_type(inkwell::AddressSpace::default());
    let i64_ptr = i64_ty.ptr_type(inkwell::AddressSpace::default());
    let f64_ptr = f64_ty.ptr_type(inkwell::AddressSpace::default());

    // fn(batch: i64, count: i64, step: i64, out: i64) -> void
    let fn_ty =
        void_ty.fn_type(&[i64_ty.into(), i64_ty.into(), i64_ty.into(), i64_ty.into()], false);
    let func = fg.module.add_function("agg_kernel", fn_ty, None);
    // llvm.sqrt.f64 intrinsic for stddev finalization
    let sqrt_ty = f64_ty.fn_type(&[f64_ty.into()], false);
    let sqrt_fn = fg
        .module
        .add_function("llvm.sqrt.f64", sqrt_ty, Some(inkwell::module::Linkage::External));
    let entry = ctx.append_basic_block(func, "entry");
    fg.builder.position_at_end(entry);

    let batch = func.get_nth_param(0).unwrap().into_int_value();
    let count = func.get_nth_param(1).unwrap().into_int_value();
    let step = func.get_nth_param(2).unwrap().into_int_value();
    let out = func.get_nth_param(3).unwrap().into_int_value();
    let _batch_ptr = fg.builder.build_int_to_ptr(batch, i8_ptr, "batch").unwrap();
    let out_ptr = fg.builder.build_int_to_ptr(out, i8_ptr, "out").unwrap();

    // ---- entry: count == 0 ? init : main ----
    let zero = i64_ty.const_zero();
    let count_is_zero =
        fg.builder.build_int_compare(IntPredicate::EQ, count, zero, "count0").unwrap();
    let init_block = ctx.append_basic_block(func, "init");
    let main_block = ctx.append_basic_block(func, "main");
    fg.builder
        .build_conditional_branch(count_is_zero, init_block, main_block)
        .unwrap();

    // ---- init block: write initial values, return ----
    fg.builder.position_at_end(init_block);
    for (col, kind) in fields.iter().enumerate() {
        write_init(&fg.builder, i64_ty, f64_ty, i64_ptr, f64_ptr, out_ptr, col, kind);
    }
    fg.builder.build_return(None).unwrap();

    // ---- main block ----
    fg.builder.position_at_end(main_block);
    // constants: write immediately
    for (col, kind) in fields.iter().enumerate() {
        let target = elem_ptr(&fg.builder, i8_ty, i64_ty, out_ptr, col);
        match kind {
            FieldKind::ConstLong(v) => {
                fg.builder.build_store(target, i64_ty.const_int(*v as u64, true)).unwrap();
            }
            FieldKind::ConstDouble(v) => {
                fg.builder.build_store(target, f64_ty.const_float(*v)).unwrap();
            }
            _ => {}
        }
    }
    // first/last: read directly
    for (col, kind) in fields.iter().enumerate() {
        match kind {
            FieldKind::First { off, out_is_long } => {
                let rec = batch; // first record at offset 0
                let target = elem_ptr(&fg.builder, i8_ty, i64_ty, out_ptr, col);
                copy_input(&fg.builder, i64_ty, f64_ty, i64_ptr, rec, *off, *out_is_long, target);
            }
            FieldKind::Last { off, out_is_long } => {
                let one = i64_ty.const_int(1, false);
                let last = fg.builder.build_int_sub(count, one, "last_idx").unwrap();
                let base = fg.builder.build_int_mul(last, step, "last_off").unwrap();
                let rec = fg.builder.build_int_add(batch, base, "last_rec").unwrap();
                let target = elem_ptr(&fg.builder, i8_ty, i64_ty, out_ptr, col);
                copy_input(&fg.builder, i64_ty, f64_ty, i64_ptr, rec, *off, *out_is_long, target);
            }
            _ => {}
        }
    }

    // ---- loop header ----
    let loop_header = ctx.append_basic_block(func, "loop_header");
    let loop_body = ctx.append_basic_block(func, "loop_body");
    let loop_latch = ctx.append_basic_block(func, "loop_latch");
    let exit_block = ctx.append_basic_block(func, "exit");
    fg.builder.build_unconditional_branch(loop_header).unwrap();

    fg.builder.position_at_end(loop_header);
    let idx_phi = fg.builder.build_phi(i64_ty, "i").unwrap();
    idx_phi.add_incoming(&[(&zero, main_block)]);
    // accumulator phis: (col, slot, phi)
    let mut acc_phis: Vec<(usize, usize, PhiValue)> = vec![];
    for (col, kind) in fields.iter().enumerate() {
        if let FieldKind::Acc { kind: acc, .. } = kind {
            let inits = acc.inits(i64_ty, f64_ty);
            for (slot, init) in inits.iter().enumerate() {
                let ty = acc.acc_types(i64_ty, f64_ty)[slot];
                let phi = fg.builder.build_phi(ty, &format!("acc_{col}_{slot}")).unwrap();
                phi.add_incoming(&[(&init.clone(), main_block)]);
                acc_phis.push((col, slot, phi));
            }
        }
    }
    let i = idx_phi.as_basic_value().into_int_value();
    let done = fg.builder.build_int_compare(IntPredicate::ULT, i, count, "done").unwrap();
    fg.builder
        .build_conditional_branch(done, loop_body, exit_block)
        .unwrap();

    // ---- loop body ----
    fg.builder.position_at_end(loop_body);
    let i = idx_phi.as_basic_value().into_int_value();
    let rec_off = fg.builder.build_int_mul(i, step, "rec_off").unwrap();
    let rec_addr = fg.builder.build_int_add(batch, rec_off, "rec_addr").unwrap();
    let rec_ptr = fg.builder.build_int_to_ptr(rec_addr, i8_ptr, "rec").unwrap();
    let mut acc_new: Vec<(usize, usize, BasicValueEnum)> = vec![];
    for (col, kind) in fields.iter().enumerate() {
        if let FieldKind::Acc { kind: acc, off, input_is_long } = kind {
            let cur: Vec<BasicValueEnum> = acc_phis
                .iter()
                .filter(|(c, _, _)| *c == col)
                .map(|(_, _, p)| p.as_basic_value())
                .collect();
            let input =
                load_input(&fg.builder, i8_ty, i64_ty, f64_ty, rec_ptr, *off, *input_is_long);
            let newv = acc_step(&fg.builder, f64_ty, *acc, &cur, input, *input_is_long);
            for (slot, v) in newv.iter().enumerate() {
                acc_new.push((col, slot, v.clone()));
            }
        }
    }
    fg.builder.build_unconditional_branch(loop_latch).unwrap();

    // ---- latch ----
    fg.builder.position_at_end(loop_latch);
    let next = fg.builder.build_int_add(i, i64_ty.const_int(1, false), "next_i").unwrap();
    for (col, slot, v) in &acc_new {
        if let Some((_, _, phi)) = acc_phis.iter_mut().find(|(c, s, _)| c == col && s == slot) {
            phi.add_incoming(&[(&v.clone(), loop_latch)]);
        }
    }
    idx_phi.add_incoming(&[(&next, loop_latch)]);
    fg.builder.build_unconditional_branch(loop_header).unwrap();

    // ---- exit: write results ----
    fg.builder.position_at_end(exit_block);
    for (col, kind) in fields.iter().enumerate() {
        let target = elem_ptr(&fg.builder, i8_ty, i64_ty, out_ptr, col);
        match kind {
            FieldKind::Acc { kind: acc, .. } => {
                let cur: Vec<BasicValueEnum> = acc_phis
                    .iter()
                    .filter(|(c, _, _)| *c == col)
                    .map(|(_, _, p)| p.as_basic_value())
                    .collect();
                let val = acc_finalize(&fg.builder, f64_ty, sqrt_fn, *acc, &cur);
                fg.builder.build_store(target, val).unwrap();
            }
            FieldKind::Skipped { out_is_long } => {
                let v: BasicValueEnum = if *out_is_long {
                    i64_ty.const_zero().into()
                } else {
                    f64_ty.const_float(0.0).into()
                };
                fg.builder.build_store(target, v).unwrap();
            }
            _ => {}
        }
    }
    fg.builder.build_return(None).unwrap();

    fg.compile::<AggFunc>("agg_kernel")
}

fn plan_field(executor: &Executor, stream: &Record, offsets: &[usize]) -> FieldKind {
    match executor {
        Executor::ConstLong(v) => FieldKind::ConstLong(*v),
        Executor::ConstDouble(v) => FieldKind::ConstDouble(*v),
        Executor::Fetch(_, _) => FieldKind::Skipped { out_is_long: true },
        Executor::Compute(func, args) => {
            let field_id = match args.index_of(0) {
                Executor::Fetch(_, fid) => *fid,
                _ => return FieldKind::Skipped { out_is_long: true },
            };
            let off = offsets.get(field_id as usize).copied().unwrap_or(usize::MAX);
            let input_is_long = matches!(stream.column(field_id).data_type(), ColumnType::Long);
            match func {
                SupportFunc::SumL => {
                    if input_is_long {
                        FieldKind::Acc { kind: AccKind::SumI, off, input_is_long }
                    } else {
                        FieldKind::Skipped { out_is_long: true }
                    }
                }
                SupportFunc::SumD => FieldKind::Acc { kind: AccKind::SumF, off, input_is_long },
                SupportFunc::Count => FieldKind::Acc { kind: AccKind::Count, off, input_is_long },
                SupportFunc::Avg => FieldKind::Acc { kind: AccKind::Avg, off, input_is_long },
                SupportFunc::MaxL => {
                    if input_is_long {
                        FieldKind::Acc { kind: AccKind::MaxI, off, input_is_long }
                    } else {
                        FieldKind::Skipped { out_is_long: true }
                    }
                }
                SupportFunc::MinL => {
                    if input_is_long {
                        FieldKind::Acc { kind: AccKind::MinI, off, input_is_long }
                    } else {
                        FieldKind::Skipped { out_is_long: true }
                    }
                }
                SupportFunc::MaxD => FieldKind::Acc { kind: AccKind::MaxF, off, input_is_long },
                SupportFunc::MinD => FieldKind::Acc { kind: AccKind::MinF, off, input_is_long },
                SupportFunc::FirstL => {
                    if input_is_long {
                        FieldKind::First { off, out_is_long: true }
                    } else {
                        FieldKind::Skipped { out_is_long: true }
                    }
                }
                SupportFunc::FirstD => FieldKind::First { off, out_is_long: false },
                SupportFunc::LastL => {
                    if input_is_long {
                        FieldKind::Last { off, out_is_long: true }
                    } else {
                        FieldKind::Skipped { out_is_long: true }
                    }
                }
                SupportFunc::LastD => FieldKind::Last { off, out_is_long: false },
                SupportFunc::Stddev => FieldKind::Acc { kind: AccKind::Stddev { sample: false }, off, input_is_long },
                SupportFunc::StddevSamp => FieldKind::Acc { kind: AccKind::Stddev { sample: true }, off, input_is_long },
                SupportFunc::Variance => FieldKind::Acc { kind: AccKind::Var { sample: false }, off, input_is_long },
                SupportFunc::VarSamp => FieldKind::Acc { kind: AccKind::Var { sample: true }, off, input_is_long },
                _ => FieldKind::Skipped { out_is_long: true },
            }
        }
    }
}

// ---------------------------------------------------------------------------
// IR helpers
// ---------------------------------------------------------------------------

/// out + col*8 (byte pointer arithmetic).
fn elem_ptr(
    builder: &Builder<'static>,
    i8_ty: inkwell::types::IntType<'static>,
    i64_ty: IntType<'static>,
    base: PointerValue<'static>,
    col: usize,
) -> PointerValue<'static> {
    unsafe {
        builder
            .build_gep(i8_ty, base, &[i64_ty.const_int((col * 8) as u64, false)], "")
            .unwrap()
    }
}

/// Copies one scalar from the input record (int offset) to the output slot.
fn copy_input(
    builder: &Builder<'static>,
    i64_ty: IntType<'static>,
    f64_ty: FloatType<'static>,
    i64_ptr: inkwell::types::PointerType<'static>,
    rec_addr: IntValue<'static>,
    off: usize,
    out_is_long: bool,
    target: PointerValue<'static>,
) {
    let addr = builder
        .build_int_add(rec_addr, i64_ty.const_int(off as u64, false), "addr")
        .unwrap();
    let i8_ptr = i64_ptr.get_context().ptr_type(inkwell::AddressSpace::default());
    let p = builder.build_int_to_ptr(addr, i8_ptr, "p").unwrap();
    let v = if out_is_long {
        builder.build_load(i64_ty, p, "v").unwrap()
    } else {
        builder.build_load(f64_ty, p, "v").unwrap()
    };
    builder.build_store(target, v).unwrap();
}

/// Loads the input value for a field: i64 or f64 per its column type.
fn load_input(
    builder: &Builder<'static>,
    i8_ty: IntType<'static>,
    i64_ty: IntType<'static>,
    _f64_ty: FloatType<'static>,
    rec_ptr: PointerValue<'static>,
    off: usize,
    input_is_long: bool,
) -> BasicValueEnum<'static> {
    // byte-accurate pointer: GEP over i8 elements with the raw byte offset
    let p = unsafe {
        builder
            .build_gep(i8_ty, rec_ptr, &[i64_ty.const_int(off as u64, false)], "fp")
            .unwrap()
    };
    // opaque pointers: build_load takes the pointee type
    if input_is_long {
        builder.build_load(i64_ty, p, "vin").unwrap()
    } else {
        builder.build_load(_f64_ty, p, "vin").unwrap()
    }
}

fn to_f64(builder: &Builder<'static>, f64_ty: FloatType<'static>, v: BasicValueEnum<'static>) -> FloatValue<'static> {
    match v {
        BasicValueEnum::FloatValue(f) => f,
        BasicValueEnum::IntValue(i) => builder.build_signed_int_to_float(i, f64_ty, "itof").unwrap(),
        other => {
            eprintln!("[jit-agg] to_f64 got unexpected value: {other:?}");
            unreachable!()
        }
    }
}

/// One accumulation step for an accumulator group. Returns the new values.
fn acc_step(
    builder: &Builder<'static>,
    f64_ty: FloatType<'static>,
    acc: AccKind,
    cur: &[BasicValueEnum<'static>],
    input: BasicValueEnum<'static>,
    _input_is_long: bool,
) -> Vec<BasicValueEnum<'static>> {
    match acc {
        AccKind::Count => vec![builder
            .build_int_add(cur[0].into_int_value(), builder.get_insert_block().unwrap().get_context().i64_type().const_int(1, false), "cnt")
            .unwrap()
            .into()],
        AccKind::SumI => {
            let v = input.into_int_value();
            vec![builder.build_int_add(cur[0].into_int_value(), v, "sum").unwrap().into()]
        }
        AccKind::SumF => {
            let vf = to_f64(builder, f64_ty, input);
            vec![builder.build_float_add(cur[0].into_float_value(), vf, "sumf").unwrap().into()]
        }
        AccKind::Avg => {
            let vf = to_f64(builder, f64_ty, input);
            let sum = builder.build_float_add(cur[0].into_float_value(), vf, "sumavg").unwrap();
            let cnt = builder.build_int_add(cur[1].into_int_value(), builder.get_insert_block().unwrap().get_context().i64_type().const_int(1, false), "cntavg").unwrap();
            vec![sum.into(), cnt.into()]
        }
        AccKind::MaxI => {
            let v = input.into_int_value();
            let gt = builder.build_int_compare(IntPredicate::SGT, v, cur[0].into_int_value(), "gt").unwrap();
            vec![builder.build_select(gt, v, cur[0].into_int_value(), "max").unwrap().into()]
        }
        AccKind::MinI => {
            let v = input.into_int_value();
            let lt = builder.build_int_compare(IntPredicate::SLT, v, cur[0].into_int_value(), "lt").unwrap();
            vec![builder.build_select(lt, v, cur[0].into_int_value(), "min").unwrap().into()]
        }
        AccKind::MaxF => {
            let vf = to_f64(builder, f64_ty, input);
            let gt = builder.build_float_compare(FloatPredicate::OGT, vf, cur[0].into_float_value(), "gtf").unwrap();
            vec![builder.build_select(gt, vf, cur[0].into_float_value(), "maxf").unwrap().into()]
        }
        AccKind::MinF => {
            let vf = to_f64(builder, f64_ty, input);
            let lt = builder.build_float_compare(FloatPredicate::OLT, vf, cur[0].into_float_value(), "ltf").unwrap();
            vec![builder.build_select(lt, vf, cur[0].into_float_value(), "minf").unwrap().into()]
        }
        AccKind::Stddev { .. } | AccKind::Var { .. } => {
            // Welford: cur = [count, mean, m2]
            let vf = to_f64(builder, f64_ty, input);
            let count = cur[0].into_float_value();
            let mean = cur[1].into_float_value();
            let m2 = cur[2].into_float_value();
            let one = f64_ty.const_float(1.0);
            let n = builder.build_float_add(count, one, "n").unwrap();
            let delta = builder.build_float_sub(vf, mean, "delta").unwrap();
            let new_mean = builder
                .build_float_add(mean, builder.build_float_div(delta, n, "dm").unwrap(), "mean")
                .unwrap();
            let dm2 = builder
                .build_float_mul(delta, builder.build_float_sub(vf, new_mean, "vm").unwrap(), "dm2")
                .unwrap();
            let new_m2 = builder.build_float_add(m2, dm2, "m2").unwrap();
            vec![n.into(), new_mean.into(), new_m2.into()]
        }
    }
    .into_iter()
    .collect()
}

/// Finalizes an accumulator group into the output value.
fn acc_finalize(
    builder: &Builder<'static>,
    f64_ty: FloatType<'static>,
    sqrt_fn: inkwell::values::FunctionValue<'static>,
    acc: AccKind,
    cur: &[BasicValueEnum<'static>],
) -> BasicValueEnum<'static> {
    match acc {
        AccKind::Avg => {
            let sum = cur[0].into_float_value();
            let cnt = cur[1].into_int_value();
            let zero = builder.get_insert_block().unwrap().get_context().i64_type().const_zero();
            let cnt_is_zero = builder.build_int_compare(IntPredicate::EQ, cnt, zero, "cz").unwrap();
            let cntf = builder.build_signed_int_to_float(cnt, f64_ty, "cntf").unwrap();
            let avg = builder.build_float_div(sum, cntf, "avg").unwrap();
            let zero_f = f64_ty.const_float(0.0);
            builder.build_select(cnt_is_zero, zero_f, avg, "avg0").unwrap().into()
        }
        AccKind::Stddev { sample } | AccKind::Var { sample } => {
            let count = cur[0].into_float_value();
            let m2 = cur[2].into_float_value();
            let one = f64_ty.const_float(1.0);
            let n_low = builder.build_float_compare(FloatPredicate::OLE, count, one, "nlow").unwrap();
            let var = if sample {
                let n1 = builder.build_float_sub(count, one, "n1").unwrap();
                builder.build_float_div(m2, n1, "vars").unwrap()
            } else {
                builder.build_float_div(m2, count, "varp").unwrap()
            };
            let result = match acc {
                AccKind::Stddev { .. } => {
                    let call = builder.build_call(sqrt_fn, &[var.into()], "sd").unwrap();
                    call.try_as_basic_value().unwrap_basic().into_float_value()
                }
                _ => var,
            };
            builder.build_select(n_low, f64_ty.const_float(0.0), result, "s0").unwrap().into()
        }
        _ => cur[0].clone(),
    }
}

/// Initial value written when count == 0 (or for skipped fields).
fn write_init(
    builder: &Builder<'static>,
    i64_ty: IntType<'static>,
    f64_ty: FloatType<'static>,
    i64_ptr: inkwell::types::PointerType<'static>,
    f64_ptr: inkwell::types::PointerType<'static>,
    out_ptr: PointerValue<'static>,
    col: usize,
    kind: &FieldKind,
) {
    let target = elem_ptr(builder, builder.get_insert_block().unwrap().get_context().i8_type(), i64_ty, out_ptr, col);
    match kind {
        FieldKind::ConstLong(v) => {
            builder.build_store(target, i64_ty.const_int(*v as u64, true)).unwrap();
        }
        FieldKind::ConstDouble(v) => {
            builder.build_store(target, f64_ty.const_float(*v)).unwrap();
        }
        FieldKind::First { out_is_long, .. } | FieldKind::Last { out_is_long, .. } => {
            let v: BasicValueEnum = if *out_is_long {
                i64_ty.const_zero().into()
            } else {
                f64_ty.const_float(0.0).into()
            };
            builder.build_store(target, v).unwrap();
        }
        FieldKind::Acc { kind: acc, .. } => {
            let inits = acc.inits(i64_ty, f64_ty);
            builder.build_store(target, inits[0]).unwrap();
        }
        FieldKind::Skipped { out_is_long } => {
            let v: BasicValueEnum = if *out_is_long {
                i64_ty.const_zero().into()
            } else {
                f64_ty.const_float(0.0).into()
            };
            builder.build_store(target, v).unwrap();
        }
    }
    let _ = i64_ptr;
    let _ = f64_ptr;
}
