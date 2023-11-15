#![allow(dead_code)]

use chrono::NaiveDate;

pub fn special_timestamp(year: i32, month: u32, day: u32, hour: u32, min: u32, sec: u32) -> u64 {
    let date = NaiveDate::from_ymd_opt(year, month, day).unwrap_or_default();
    let date_time = date.and_hms_opt(hour, min, sec).unwrap_or_default();
    date_time.timestamp_millis() as u64
}
