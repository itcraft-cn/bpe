use strum_macros::EnumString;

#[derive(Debug, Clone, EnumString)]
pub(crate) enum SupportFunc {
    // calc func
    #[strum(ascii_case_insensitive)]
    Add,
    #[strum(ascii_case_insensitive)]
    Sub,
    #[strum(ascii_case_insensitive)]
    Mul,
    #[strum(ascii_case_insensitive)]
    Div,
    #[strum(ascii_case_insensitive)]
    Mod,
    // aggregate func
    #[strum(ascii_case_insensitive)]
    MinL,
    #[strum(ascii_case_insensitive)]
    MaxL,
    #[strum(ascii_case_insensitive)]
    SumL,
    #[strum(ascii_case_insensitive)]
    Count,
    #[strum(ascii_case_insensitive)]
    MinD,
    #[strum(ascii_case_insensitive)]
    MaxD,
    #[strum(ascii_case_insensitive)]
    SumD,
    #[strum(ascii_case_insensitive)]
    Avg,
    #[strum(ascii_case_insensitive)]
    FirstL,
    #[strum(ascii_case_insensitive)]
    FirstD,
    #[strum(ascii_case_insensitive)]
    LastL,
    #[strum(ascii_case_insensitive)]
    LastD,
}
