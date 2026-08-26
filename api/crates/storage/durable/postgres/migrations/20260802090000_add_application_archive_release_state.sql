alter table applications
    add column release_version bigint not null default 0,
    add column release_digest text null;

alter table applications
    add constraint applications_archive_release_state_valid
    check (
        (release_version = 0 and release_digest is null)
        or (
            release_version >= 1
            and release_digest ~ '^[0-9a-f]{64}$'
        )
    );
