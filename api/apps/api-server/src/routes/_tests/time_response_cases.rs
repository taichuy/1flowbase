use axum::{http::StatusCode, response::IntoResponse};
use time::{Date, Month, OffsetDateTime, Time, UtcOffset};

use crate::error_response::ApiError;

pub(crate) fn assert_timestamp_mapping(
    mut map: impl FnMut(OffsetDateTime) -> Result<Vec<String>, ApiError>,
) {
    let cases = [
        (
            Date::from_calendar_date(2026, Month::July, 11)
                .unwrap()
                .with_time(Time::from_hms_nano(10, 55, 28, 123_456_789).unwrap())
                .assume_utc(),
            "2026-07-11T10:55:28.123456789Z",
        ),
        (
            Date::from_calendar_date(0, Month::January, 1)
                .unwrap()
                .midnight()
                .assume_utc(),
            "0000-01-01T00:00:00Z",
        ),
        (
            Date::from_calendar_date(9999, Month::December, 31)
                .unwrap()
                .with_time(Time::from_hms(23, 59, 59).unwrap())
                .assume_utc(),
            "9999-12-31T23:59:59Z",
        ),
        (
            Date::from_calendar_date(2026, Month::July, 11)
                .unwrap()
                .midnight()
                .assume_offset(UtcOffset::from_hms(5, 30, 0).unwrap()),
            "2026-07-11T00:00:00+05:30",
        ),
    ];
    for (value, expected) in cases {
        let fields = map(value).expect("supported RFC3339 response timestamp");
        assert!(!fields.is_empty());
        for field in fields {
            assert_eq!(field, expected);
        }
    }
    let invalid = [
        Date::from_calendar_date(-1, Month::January, 1)
            .unwrap()
            .midnight()
            .assume_utc(),
        Date::from_calendar_date(2026, Month::July, 11)
            .unwrap()
            .midnight()
            .assume_offset(UtcOffset::from_hms(5, 30, 1).unwrap()),
    ];
    for value in invalid {
        let error =
            map(value).expect_err("unrepresentable timestamp must fail the response mapper");
        assert!(error.0.downcast_ref::<time::error::Format>().is_some());
        assert_eq!(
            error.into_response().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
