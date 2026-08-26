create table network_egress_route_pool_members (
    route_id uuid not null references network_egress_routes(id) on delete cascade,
    pool_member_id uuid not null,
    sequence integer not null check (sequence >= 0),
    primary key (route_id, pool_member_id),
    unique (route_id, sequence),
    constraint network_egress_route_pool_members_member_fk
        foreign key (pool_member_id)
        references network_egress_pool_members(id)
        on delete restrict
);

insert into network_egress_route_pool_members (route_id, pool_member_id, sequence)
select
    routes.id,
    members.id,
    row_number() over (
        partition by routes.id
        order by members.sequence asc, members.id asc
    )::integer - 1
from network_egress_routes routes
join network_egress_pool_members members on members.pool_id = routes.pool_id;

create index network_egress_route_pool_members_member_idx
    on network_egress_route_pool_members (pool_member_id);
