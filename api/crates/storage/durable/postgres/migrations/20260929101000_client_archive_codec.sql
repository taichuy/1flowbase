-- Version 0 remains the original frames JSONB + uncompressed bytes layout.
-- SQL runtime_event_original_payload still resolves referenced legacy_json
-- parts from that layout; only verified, unreferenced wire parts may be moved.
alter table client_trajectory_archive_parts
    add column codec_version smallint not null default 0,
    add column frame_directory bytea,
    add column raw_byte_length bigint,
    add column raw_checksum bytea,
    add constraint client_archive_codec_nonnegative check(codec_version >= 0),
    add constraint client_archive_raw_length_nonnegative check(raw_byte_length >= 0),
    add constraint client_archive_checksum_size check(raw_checksum is null or octet_length(raw_checksum)=32),
    add constraint client_archive_codec_columns check(
        (codec_version=0 and frame_directory is null and raw_byte_length is null and raw_checksum is null)
        or (codec_version>0 and frame_directory is not null and raw_byte_length is not null and raw_checksum is not null)
    );
