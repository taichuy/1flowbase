with ranked_publications as (
    select
        id,
        row_number() over (
            partition by application_id
            order by active desc, version_sequence desc, created_at desc, id desc
        ) as publication_rank
    from application_publication_versions
)
delete from application_publication_versions publication
using ranked_publications ranked
where publication.id = ranked.id
  and ranked.publication_rank > 1;

update application_publication_versions
set active = true,
    version_sequence = 1;

drop index if exists application_publication_versions_active_idx;
drop index if exists application_publication_versions_application_sequence_idx;

create unique index if not exists application_publication_versions_application_id_idx
    on application_publication_versions (application_id);
