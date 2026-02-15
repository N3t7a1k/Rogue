use crate::types::{DateTimeWithPrecision, Precision};
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, Timelike, TimeZone, Utc};
use windows::Win32::Foundation::FILETIME;
use windows_sys::Win32::Foundation::SYSTEMTIME;

pub fn merge_datetime(original: &DateTime<Utc>, new: &DateTimeWithPrecision) -> DateTime<Utc> {
    match new.precision {
        Precision::DateOnly => {
            new.datetime.with_time(original.time()).unwrap()
               .with_nanosecond(original.nanosecond()).unwrap()
        }
        Precision::Seconds => {
            new.datetime.with_nanosecond(original.nanosecond()).unwrap_or(new.datetime)
        }
        Precision::Milliseconds => {
            new.datetime
        }
    }
}

pub fn string_to_datetime_with_precision(input: &str) -> Option<DateTimeWithPrecision> {
    if let Ok(dt) = NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M:%S%.f") {
        return Some(DateTimeWithPrecision {
            datetime: Utc.from_utc_datetime(&dt),
            precision: Precision::Milliseconds,
        });
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M:%S") {
        return Some(DateTimeWithPrecision {
            datetime: Utc.from_utc_datetime(&dt),
            precision: Precision::Seconds,
        });
    }
    if let Ok(d) = NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        let dt = Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap());
        return Some(DateTimeWithPrecision {
            datetime: dt,
            precision: Precision::DateOnly,
        });
    }
    None
}

pub fn systemtime_to_datetime(st: &SYSTEMTIME) -> DateTime<Utc> {
    let naive_date = NaiveDate::from_ymd_opt(
        st.wYear as i32, 
        st.wMonth as u32, 
        st.wDay as u32
    ).unwrap_or_default();

    let naive_datetime = naive_date.and_hms_milli_opt(
        st.wHour as u32, 
        st.wMinute as u32, 
        st.wSecond as u32, 
        st.wMilliseconds as u32
    ).unwrap_or_default();


    Utc.from_utc_datetime(&naive_datetime)
}

pub fn filetime_to_datetime(ft: &FILETIME) -> DateTime<Utc> {
    let duration = ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64);

    if duration == 0 {
        return Utc.timestamp_opt(0, 0).unwrap();
    }

    let intervals_per_sec = 10_000_000;
    let windows_epoch_offset = 11_644_473_600;
    let total_seconds = duration / intervals_per_sec;
    let nanos = (duration % intervals_per_sec) * 100;
    let unix_seconds = (total_seconds as i64) - windows_epoch_offset;

    Utc.timestamp_opt(unix_seconds, nanos as u32).unwrap()
}

pub fn filetime_to_string(ft: &FILETIME) -> String {
    let dt = filetime_to_datetime(ft);
    let dt_local: DateTime<Local> = DateTime::from(dt);
    dt_local.format("%Y-%m-%d %H:%M:%S%.3f").to_string()
}

pub fn datetime_to_filetime(dt: DateTime<Utc>) -> FILETIME {
    let windows_epoch_offset = 11_644_473_600i64;
    let intervals_per_sec = 10_000_000;

    let unix_seconds = dt.timestamp();
    let nanos = dt.timestamp_subsec_nanos() as u64;

    let total_seconds = unix_seconds + windows_epoch_offset;
    let total_ticks = (total_seconds as u64 * intervals_per_sec) + (nanos / 100);

    FILETIME {
        dwLowDateTime: total_ticks as u32,
        dwHighDateTime: (total_ticks >> 32) as u32,
    }
}