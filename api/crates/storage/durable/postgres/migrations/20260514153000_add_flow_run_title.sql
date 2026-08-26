alter table flow_runs
    add column title text not null default '';

update flow_runs
set title = left(
    coalesce(
        nullif(trim(coalesce(input_payload #>> '{node-start,query}', input_payload ->> 'query')), ''),
        'Untitled run'
    ),
    255
)
where title = '';
