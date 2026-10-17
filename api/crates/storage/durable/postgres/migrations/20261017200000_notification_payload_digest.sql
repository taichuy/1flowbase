-- Retain compact replay identity after completed typed-notification body release.
alter table lifecycle_outbox
    add column notification_payload_digest bytea
        check (notification_payload_digest is null or octet_length(notification_payload_digest) = 32);
