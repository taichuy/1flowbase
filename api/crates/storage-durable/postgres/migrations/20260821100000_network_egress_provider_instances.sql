alter table network_egress_providers
    drop constraint network_egress_providers_installation_id_key,
    add column description text not null default '';

alter table network_egress_pools
    add column owner_provider_id uuid unique references network_egress_providers(id) on delete cascade;

create index network_egress_pools_owner_provider_idx
    on network_egress_pools (owner_provider_id)
    where owner_provider_id is not null;
