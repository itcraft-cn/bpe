use strum_macros::{Display, EnumString};

/// Supported SQL functions, referenced in SQL as `_name(...)` (e.g. `_suml`, `_abs`).
/// `Display` yields the canonical (snake_case) name used to resolve the JIT module
/// function (`func_{name}`); `EnumString` matches SQL names case-insensitively.
#[derive(Debug, Clone, EnumString, Display)]
#[strum(ascii_case_insensitive)]
pub(crate) enum SupportFunc {
    // calc func
    #[strum(serialize = "add")]
    Add,
    #[strum(serialize = "sub")]
    Sub,
    #[strum(serialize = "mul")]
    Mul,
    #[strum(serialize = "div")]
    Div,
    #[strum(serialize = "mod")]
    Mod,
    // scalar func (1-arg)
    #[strum(serialize = "abs")]
    Abs,
    #[strum(serialize = "ceil")]
    Ceil,
    #[strum(serialize = "floor")]
    Floor,
    #[strum(serialize = "round")]
    Round,
    #[strum(serialize = "sqrt")]
    Sqrt,
    #[strum(serialize = "exp")]
    Exp,
    #[strum(serialize = "ln")]
    Ln,
    #[strum(serialize = "log10")]
    Log10,
    #[strum(serialize = "sign")]
    Sign,
    #[strum(serialize = "trunc")]
    Trunc,
    #[strum(serialize = "to_long")]
    ToLong,
    #[strum(serialize = "to_double")]
    ToDouble,
    // scalar func (2-arg)
    #[strum(serialize = "pow")]
    Pow,
    #[strum(serialize = "greatest")]
    Greatest,
    #[strum(serialize = "least")]
    Least,
    // aggregate func
    #[strum(serialize = "minl")]
    MinL,
    #[strum(serialize = "maxl")]
    MaxL,
    #[strum(serialize = "suml")]
    SumL,
    #[strum(serialize = "count")]
    Count,
    #[strum(serialize = "mind")]
    MinD,
    #[strum(serialize = "maxd")]
    MaxD,
    #[strum(serialize = "sumd")]
    SumD,
    #[strum(serialize = "avg")]
    Avg,
    #[strum(serialize = "firstl")]
    FirstL,
    #[strum(serialize = "firstd")]
    FirstD,
    #[strum(serialize = "lastl")]
    LastL,
    #[strum(serialize = "lastd")]
    LastD,
    // aggregate func (variance/stddev, output is f64)
    #[strum(serialize = "stddev")]
    Stddev,
    #[strum(serialize = "stddev_samp")]
    StddevSamp,
    #[strum(serialize = "variance")]
    Variance,
    #[strum(serialize = "var_samp")]
    VarSamp,
}
