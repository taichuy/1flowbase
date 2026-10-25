-- Communication state has dedicated service-only access, never generic model CRUD.
CREATE TABLE communication_bindings (
    instance_id uuid PRIMARY KEY REFERENCES notification_instances(id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    actor_user_id uuid NOT NULL,
    config_json jsonb NOT NULL,
    plugin_mcp_instance_id text NOT NULL,
    revision bigint NOT NULL DEFAULT 1,
    encrypted_cursor jsonb
);
CREATE TABLE communication_conversations (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    instance_id uuid NOT NULL,
    external_identity text NOT NULL,
    external_conversation_key text NOT NULL,
    sender_display_name text,
    UNIQUE(workspace_id,instance_id,external_identity,external_conversation_key)
);
-- History intentionally survives channel/user deletion. Provider keys dedup per channel+sender.
CREATE TABLE communication_messages (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    instance_id uuid NOT NULL,
    conversation_id uuid NOT NULL REFERENCES communication_conversations(id),
    external_identity text NOT NULL,
    external_message_key text NOT NULL,
    received_at_ms bigint NOT NULL,
    content text,
    processing_status text NOT NULL DEFAULT 'pending' CHECK(processing_status IN ('pending','processing','succeeded','failed')),
    run_id uuid,
    final_reply text,
    send_status text NOT NULL DEFAULT 'none' CHECK(send_status IN ('none','pending','sending','succeeded','failed','unknown')),
    failure_code text,
    claim_id uuid,
    claim_expires_at timestamptz,
    UNIQUE(workspace_id,instance_id,external_identity,external_message_key)
);
CREATE UNIQUE INDEX communication_messages_run_idx ON communication_messages(run_id) WHERE run_id IS NOT NULL;
CREATE INDEX communication_messages_queue_idx ON communication_messages(workspace_id,instance_id,conversation_id,id);
CREATE TABLE communication_attachments (
    id uuid PRIMARY KEY,
    message_id uuid NOT NULL REFERENCES communication_messages(id),
    provider_attachment_id text NOT NULL,
    file_name text NOT NULL,
    media_type text NOT NULL,
    size_bytes bigint CHECK(size_bytes >= 0),
    encrypted_reference jsonb NOT NULL,
    status text NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','ready','failed')),
    file_table_id uuid,
    file_record_id uuid,
    failure_code text,
    UNIQUE(message_id,provider_attachment_id),
    CHECK((status = 'ready') = (file_table_id IS NOT NULL AND file_record_id IS NOT NULL))
);
CREATE INDEX communication_files_message_idx ON communication_attachments(message_id,id);
