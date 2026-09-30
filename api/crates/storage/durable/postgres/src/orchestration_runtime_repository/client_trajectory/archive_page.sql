-- Both candidates and their block are selected in one MVCC statement. Publication
-- cannot leave a reader between the old directory and its committed replacement.
with page as (
    (select p.request_id,p.first_sequence,p.last_sequence,p.frames,p.bytes,p.codec_version,
        p.frame_directory,p.raw_byte_length,p.raw_checksum,p.block_id,p.block_offset,
        null::smallint as segment_codec_version,null::uuid[] as part_ids,null::bigint as frame_count
     from client_trajectory_archive_parts p
     where p.request_id=$1 and p.last_sequence>$2
       and ($3::bigint is null or p.last_sequence>$3)
     order by p.first_sequence limit 1)
    union all
    (select s.request_id,s.first_sequence,s.last_sequence,'[]'::jsonb,''::bytea,4::smallint,
        s.directory,s.directory_raw_length,s.directory_checksum,s.block_id,null::bigint,
        s.codec_version,s.part_ids,s.frame_count
     from client_trajectory_archive_segments s
     where s.request_id=$1 and s.last_sequence>$2
       and ($3::bigint is null or s.last_sequence>$3)
     order by s.first_sequence limit 1)
)
select p.*,b.codec_version as block_codec_version,
    case when b.block_id=$4::uuid then null::bytea else b.bytes end as block_bytes,
    b.raw_byte_length as block_raw_length,b.raw_checksum as block_checksum
from page p left join client_trajectory_archive_blocks b
    on b.request_id=p.request_id and b.block_id=p.block_id
order by p.first_sequence limit 1
