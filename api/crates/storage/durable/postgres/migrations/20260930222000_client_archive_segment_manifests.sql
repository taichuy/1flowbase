-- No retained data is moved by this migration. New sealing and explicitly scoped
-- maintenance publish an authenticated manifest before retiring its staging rows.
create table client_trajectory_archive_segments (
    block_id uuid primary key,
    request_id uuid not null references client_trajectory_archive_heads(request_id) on delete cascade,
    first_sequence bigint not null check(first_sequence>0),
    last_sequence bigint not null check(last_sequence>=first_sequence),
    frame_count bigint not null check(frame_count=last_sequence-first_sequence+1),
    codec_version smallint not null check(codec_version=1),
    part_ids uuid[] not null check(cardinality(part_ids)>0),
    directory bytea not null,
    directory_raw_length bigint not null check(directory_raw_length>0),
    directory_checksum bytea not null check(octet_length(directory_checksum)=32),
    unique(request_id,first_sequence),
    foreign key(request_id,block_id) references client_trajectory_archive_blocks(request_id,block_id)
);
create index client_archive_segment_page on client_trajectory_archive_segments(request_id,last_sequence);
-- Original receipt identities survive sealing; retries use the indexed manifest
-- owner lookup instead of retaining one heap/index row for every tiny part.
create index client_archive_segment_part_identity on client_trajectory_archive_segments using gin(part_ids);
