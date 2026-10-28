-- Additive durable session/turn protocol. Stable external-chat/history UUIDs are unchanged.
ALTER TABLE communication_conversations
    ADD COLUMN session_epoch bigint NOT NULL DEFAULT 0 CHECK(session_epoch >= 0),
    ADD COLUMN selected_model text;

CREATE TABLE communication_sessions (
    id uuid PRIMARY KEY,
    scope_id uuid NOT NULL,
    instance_id uuid NOT NULL,
    external_identity text NOT NULL,
    conversation_id uuid NOT NULL,
    epoch bigint NOT NULL CHECK(epoch > 0),
    active boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(scope_id,instance_id,external_identity,conversation_id,epoch),
    UNIQUE(scope_id,instance_id,external_identity,conversation_id,id,epoch)
);
CREATE UNIQUE INDEX communication_session_active_idx
    ON communication_sessions(scope_id,instance_id,external_identity,conversation_id) WHERE active;

CREATE TABLE communication_reply_turns (
    id uuid PRIMARY KEY,
    scope_id uuid NOT NULL,
    instance_id uuid NOT NULL,
    external_identity text NOT NULL,
    conversation_id uuid NOT NULL,
    session_id uuid NOT NULL,
    session_epoch bigint NOT NULL,
    anchor_input_id uuid NOT NULL UNIQUE,
    state text NOT NULL DEFAULT 'open' CHECK(state IN ('open','sealed','failed','invalidated')),
    selected_model text,
    run_id uuid,
    received_watermark bigint NOT NULL DEFAULT 0,
    consumed_watermark bigint NOT NULL DEFAULT 0,
    checkpoint_revision bigint NOT NULL DEFAULT 0 CHECK(checkpoint_revision >= 0),
    checkpoint_json jsonb,
    processing_status text NOT NULL DEFAULT 'pending' CHECK(processing_status IN ('pending','processing','succeeded','failed')),
    send_status text NOT NULL DEFAULT 'none' CHECK(send_status IN ('none','pending','sending','succeeded','failed','unknown')),
    failure_code text,
    outbound_message_id uuid UNIQUE,
    claim_id uuid,
    claim_expires_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK(received_watermark >= consumed_watermark AND consumed_watermark >= 0),
    CHECK((checkpoint_revision = 0) = (checkpoint_json IS NULL)),
    FOREIGN KEY(scope_id,instance_id,external_identity,conversation_id,session_id,session_epoch)
        REFERENCES communication_sessions(scope_id,instance_id,external_identity,conversation_id,id,epoch)
);
CREATE UNIQUE INDEX communication_turn_open_idx ON communication_reply_turns(session_id) WHERE state='open';
CREATE UNIQUE INDEX communication_turn_run_idx ON communication_reply_turns(run_id) WHERE run_id IS NOT NULL;
CREATE INDEX communication_turn_queue_idx ON communication_reply_turns(scope_id,instance_id,id);

-- Provider inputs and timeline rows deliberately retain their existing deletion-independent
-- history semantics. Membership is checked with the complete scope in every adapter write.
ALTER TABLE communication_inputs
    ADD COLUMN turn_id uuid,
    ADD COLUMN turn_sequence bigint,
    ADD COLUMN admission_status text NOT NULL DEFAULT 'legacy'
        CHECK(admission_status IN ('pending','admitted','excluded','legacy')),
    ADD CONSTRAINT communication_input_membership CHECK(
        (admission_status='admitted' AND turn_id IS NOT NULL AND turn_sequence IS NOT NULL AND turn_sequence > 0)
        OR (admission_status<>'admitted' AND turn_id IS NULL AND turn_sequence IS NULL));
CREATE UNIQUE INDEX communication_input_turn_sequence_idx ON communication_inputs(turn_id,turn_sequence)
    WHERE turn_id IS NOT NULL;
CREATE INDEX communication_input_admission_idx ON communication_inputs(scope_id,instance_id,conversation_id,id)
    WHERE admission_status='pending';
-- Mark old rows without rewriting their timestamps or processing facts. Admission accepts
-- legacy rows only when still pending, unclaimed and without a run/send association.
ALTER TABLE communication_inputs ALTER COLUMN admission_status SET DEFAULT 'pending';

CREATE TABLE communication_session_rotations (
    scope_id uuid NOT NULL,
    instance_id uuid NOT NULL,
    external_identity text NOT NULL,
    conversation_id uuid NOT NULL,
    receipt_key text NOT NULL CHECK(length(receipt_key)>0),
    result_json jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY(scope_id,instance_id,external_identity,conversation_id,receipt_key)
);
CREATE INDEX communication_sessions_scope_created_idx ON communication_sessions(scope_id,created_at,id);
CREATE INDEX communication_turns_scope_created_idx ON communication_reply_turns(scope_id,created_at,id);
CREATE INDEX communication_rotations_scope_created_idx ON communication_session_rotations(scope_id,created_at);
CREATE TRIGGER communication_sessions_updated_at BEFORE UPDATE ON communication_sessions
    FOR EACH ROW EXECUTE FUNCTION communication_touch_updated_at();
CREATE TRIGGER communication_turns_updated_at BEFORE UPDATE ON communication_reply_turns
    FOR EACH ROW EXECUTE FUNCTION communication_touch_updated_at();
