-- Privacy masking runtime policy snapshots and zero-plaintext audit logs.

CREATE TABLE IF NOT EXISTS privacy_masking_runtime_policies (
    installation_id UUID NOT NULL REFERENCES extension_installations(id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL,
    policy_version BIGINT NOT NULL DEFAULT 1,
    policy_json JSONB NOT NULL,
    updated_by UUID NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (installation_id, workspace_id)
);

CREATE INDEX IF NOT EXISTS idx_privacy_masking_runtime_policies_workspace
    ON privacy_masking_runtime_policies (workspace_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS privacy_masking_logs (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL,
    installation_id UUID NULL,
    flow_run_id UUID NULL,
    application_id UUID NULL,
    provider_instance_id UUID NULL,
    subject_kind TEXT NOT NULL CHECK (subject_kind IN ('agent_flow', 'model_provider_instance')),
    subject_id TEXT NOT NULL,
    subject_name TEXT NOT NULL DEFAULT '',
    model_id TEXT NOT NULL DEFAULT '',
    protocol TEXT NOT NULL DEFAULT '',
    restore_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    hit_count INTEGER NOT NULL DEFAULT 0,
    restored_count INTEGER NOT NULL DEFAULT 0,
    matched_rules_json JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_privacy_masking_logs_workspace_created
    ON privacy_masking_logs (workspace_id, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_privacy_masking_logs_workspace_subject
    ON privacy_masking_logs (workspace_id, subject_kind, subject_id, created_at DESC, id DESC);
