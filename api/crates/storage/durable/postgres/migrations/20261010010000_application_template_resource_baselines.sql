-- Additive: historical installations intentionally have no applied baseline.
CREATE TABLE application_template_resource_baselines (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    scope_id uuid NOT NULL CHECK (scope_id = workspace_id),
    template_id text NOT NULL,
    kind text NOT NULL,
    source_id text NOT NULL,
    target_id text NOT NULL,
    generation bigint NOT NULL DEFAULT 0 CHECK (generation >= 0),
    applied_fingerprint text,
    pending_operation_id uuid,
    pending_expected_fingerprint text,
    pending_desired_fingerprint text,
    committed_operation_id uuid,
    committed_fingerprint text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, template_id, kind, source_id),
    CHECK ((pending_operation_id IS NULL) = (pending_desired_fingerprint IS NULL)),
    CHECK (pending_operation_id IS NOT NULL OR pending_expected_fingerprint IS NULL),
    CHECK (committed_operation_id IS NULL OR committed_operation_id = pending_operation_id),
    CHECK (committed_operation_id IS NULL OR pending_operation_id IS NOT NULL),
    CHECK ((committed_operation_id IS NULL) = (committed_fingerprint IS NULL))
);

CREATE INDEX application_template_resource_baselines_scope_created_id_idx
    ON application_template_resource_baselines (scope_id, created_at, id);
