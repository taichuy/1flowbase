create index if not exists frontstage_block_nodes_scope_created_id_idx
  on frontstage_block_nodes (scope_id, created_at, id);
