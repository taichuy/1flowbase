use domain::SessionRecord;
use time::{Duration, OffsetDateTime};

pub(crate) fn session_ttl(expires_at_unix: i64) -> Duration {
    Duration::seconds(expires_at_unix - OffsetDateTime::now_utc().unix_timestamp())
}

pub(crate) fn is_session_expired(session: &SessionRecord) -> bool {
    session.expires_at_unix <= OffsetDateTime::now_utc().unix_timestamp()
}
