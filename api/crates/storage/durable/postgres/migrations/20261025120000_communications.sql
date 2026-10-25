-- Communication state has dedicated service-only access, never generic model CRUD.
-- The channel keeps its existing workspace identity; new physical records use scope_id.
CREATE UNIQUE INDEX notification_instances_workspace_identity_idx ON notification_instances(workspace_id,id);
CREATE TABLE communication_bindings (
    instance_id uuid PRIMARY KEY,
    scope_id uuid NOT NULL,
    actor_user_id uuid NOT NULL,
    config_json jsonb NOT NULL,
    plugin_mcp_instance_id text NOT NULL,
    revision bigint NOT NULL DEFAULT 1,
    encrypted_cursor jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY(scope_id,instance_id) REFERENCES notification_instances(workspace_id,id) ON DELETE CASCADE
);
CREATE TABLE communication_conversations (
    id uuid PRIMARY KEY,
    scope_id uuid NOT NULL,
    instance_id uuid NOT NULL,
    external_identity text NOT NULL,
    external_conversation_key text NOT NULL,
    sender_display_name text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(scope_id,instance_id,external_identity,id),
    UNIQUE(scope_id,instance_id,external_identity,external_conversation_key)
);
-- History intentionally survives channel/user deletion. Provider keys dedup per channel+sender.
CREATE TABLE communication_messages (
    id uuid PRIMARY KEY,
    scope_id uuid NOT NULL,
    instance_id uuid NOT NULL,
    conversation_id uuid NOT NULL,
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
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(scope_id,id),
    FOREIGN KEY(scope_id,instance_id,external_identity,conversation_id) REFERENCES communication_conversations(scope_id,instance_id,external_identity,id),
    UNIQUE(scope_id,instance_id,external_identity,external_message_key)
);
CREATE UNIQUE INDEX communication_messages_run_idx ON communication_messages(run_id) WHERE run_id IS NOT NULL;
CREATE INDEX communication_messages_queue_idx ON communication_messages(scope_id,instance_id,conversation_id,id);
CREATE TABLE communication_attachments (
    id uuid PRIMARY KEY,
    scope_id uuid NOT NULL,
    message_id uuid NOT NULL,
    provider_attachment_id text NOT NULL,
    file_name text NOT NULL,
    media_type text NOT NULL,
    size_bytes bigint CHECK(size_bytes >= 0),
    encrypted_reference jsonb NOT NULL,
    status text NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','ready','failed')),
    file_table_id uuid,
    file_record_id uuid,
    failure_code text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY(scope_id,message_id) REFERENCES communication_messages(scope_id,id),
    UNIQUE(message_id,provider_attachment_id),
    CHECK((status = 'ready') = (file_table_id IS NOT NULL AND file_record_id IS NOT NULL))
);
CREATE INDEX communication_files_message_idx ON communication_attachments(message_id,id);

CREATE INDEX communication_bindings_scope_created_idx ON communication_bindings(scope_id,created_at,instance_id);
CREATE INDEX communication_conversations_scope_created_idx ON communication_conversations(scope_id,created_at,id);
CREATE INDEX communication_messages_scope_created_idx ON communication_messages(scope_id,created_at,id);
CREATE INDEX communication_attachments_scope_created_idx ON communication_attachments(scope_id,created_at,id);

-- All four records have mutable state. Keep audit time current for every writer, including
-- lease renewal, receive cursor updates and attachment recovery, without changing public DTOs.
CREATE FUNCTION communication_touch_updated_at() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    NEW.updated_at := clock_timestamp();
    RETURN NEW;
END;
$$;
CREATE TRIGGER communication_bindings_updated_at BEFORE UPDATE ON communication_bindings FOR EACH ROW EXECUTE FUNCTION communication_touch_updated_at();
CREATE TRIGGER communication_conversations_updated_at BEFORE UPDATE ON communication_conversations FOR EACH ROW EXECUTE FUNCTION communication_touch_updated_at();
CREATE TRIGGER communication_messages_updated_at BEFORE UPDATE ON communication_messages FOR EACH ROW EXECUTE FUNCTION communication_touch_updated_at();
CREATE TRIGGER communication_attachments_updated_at BEFORE UPDATE ON communication_attachments FOR EACH ROW EXECUTE FUNCTION communication_touch_updated_at();
