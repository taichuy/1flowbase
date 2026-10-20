-- Credit ledger pages may omit user_id; keep workspace chronology contiguous.
create index runtime_credit_ledger_workspace_created_id_idx
    on runtime_credit_ledger (workspace_id, created_at desc, id desc);
