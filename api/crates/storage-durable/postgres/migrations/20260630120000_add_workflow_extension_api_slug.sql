alter table application_api_mappings
    add column if not exists extension_slug text null;

update application_api_mappings
   set extension_slug = nullif(mapping_config #>> '{extension,slug}', '')
 where extension_slug is null
   and mapping_config ? 'extension';

create unique index if not exists application_api_mappings_extension_slug_uidx
    on application_api_mappings (extension_slug)
    where extension_slug is not null;

alter table application_publication_versions
    add column if not exists extension_slug text null;

update application_publication_versions
   set extension_slug = nullif(mapping_snapshot #>> '{extension,slug}', '')
 where extension_slug is null
   and mapping_snapshot ? 'extension';

create unique index if not exists application_publication_versions_extension_slug_uidx
    on application_publication_versions (extension_slug)
    where extension_slug is not null;
