use chrono::{DateTime, FixedOffset, Local};

pub trait DateTimeExt {
    fn now() -> DateTime<FixedOffset>;
}

impl DateTimeExt for DateTime<FixedOffset> {
    /// Gets current date and time and timezone in DateTime<FixedOffset>.
    fn now() -> DateTime<FixedOffset> {
        let local_time = Local::now();
        local_time.with_timezone(local_time.offset())
    }
}
