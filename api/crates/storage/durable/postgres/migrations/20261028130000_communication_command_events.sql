-- Commands have a durable independent control lane. Plugin execution uses lifecycle_outbox;
-- builtin execution claims this inbox. Neither handler ACK means provider delivery.
CREATE TABLE communication_command_events (
    event_id uuid PRIMARY KEY,
    input_id uuid NOT NULL UNIQUE,
    scope_id uuid NOT NULL,
    instance_id uuid NOT NULL,
    external_identity text NOT NULL,
    conversation_id uuid NOT NULL,
    session_id uuid NOT NULL,
    request_json jsonb NOT NULL,
    installation_id uuid,
    contribution_id text,
    graph_fingerprint text,
    subscriber_id text,
    handler_id text,
    handler_version text,
    effect_claim_id uuid,
    effect_claim_expires_at timestamptz,
    reply_json jsonb,
    failure_code text,
    response_session_id uuid,
    send_status text NOT NULL DEFAULT 'none' CHECK(send_status IN ('none','pending','sending','succeeded','failed','unknown')),
    send_claim_id uuid,
    send_claim_expires_at timestamptz,
    outbound_message_id uuid UNIQUE,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK ((installation_id IS NULL AND contribution_id IS NULL AND graph_fingerprint IS NULL AND subscriber_id IS NULL AND handler_id IS NULL AND handler_version IS NULL)
        OR (installation_id IS NOT NULL AND contribution_id IS NOT NULL AND graph_fingerprint IS NOT NULL AND subscriber_id IS NOT NULL AND handler_id IS NOT NULL AND handler_version IS NOT NULL)),
    CHECK ((reply_json IS NULL AND response_session_id IS NULL AND send_status='none') OR (reply_json IS NOT NULL AND response_session_id IS NOT NULL AND send_status<>'none'))
);
CREATE INDEX communication_command_queue_idx ON communication_command_events(scope_id,instance_id,event_id) WHERE reply_json IS NULL;
CREATE INDEX communication_command_response_idx ON communication_command_events(scope_id,instance_id,event_id) WHERE send_status IN ('pending','sending');
CREATE INDEX communication_command_scope_created_idx ON communication_command_events(scope_id,created_at,event_id);
CREATE TRIGGER communication_command_updated_at BEFORE UPDATE ON communication_command_events
    FOR EACH ROW EXECUTE FUNCTION communication_touch_updated_at();
