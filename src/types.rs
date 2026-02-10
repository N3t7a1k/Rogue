use chrono::{DateTime, Utc};

pub enum Precision {
    DateOnly,
    Seconds,
    Milliseconds,
}

pub struct DateTimeWithPrecision {
    pub datetime: DateTime<Utc>,
    pub precision: Precision,
}

