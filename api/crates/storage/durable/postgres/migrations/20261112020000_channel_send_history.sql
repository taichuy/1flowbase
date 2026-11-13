-- A proactive channel send has no inbound input. Keep the existing unique input/reply slot.
ALTER TABLE communication_messages ALTER COLUMN input_id DROP NOT NULL;

CREATE TABLE communication_message_sends (
    send_log_id uuid PRIMARY KEY REFERENCES notification_send_logs(id),
    scope_id uuid NOT NULL,
    instance_id uuid NOT NULL,
    message_id uuid REFERENCES communication_messages(id),
    conversation_id uuid,
    input_id uuid,
    run_id uuid,
    target_json jsonb,
    receipt_json jsonb,
    send_status text NOT NULL CHECK(send_status IN ('sending','succeeded','failed','unknown')),
    failure_code text,
    created_at timestamptz NOT NULL DEFAULT now(),
    CHECK ((message_id IS NULL) = (conversation_id IS NULL))
);
CREATE INDEX communication_message_sends_history_idx
    ON communication_message_sends(scope_id,message_id,created_at DESC);
