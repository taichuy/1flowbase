-- A quotation belongs to the original input, independent of command admission or reply sends.
ALTER TABLE communication_inputs ADD COLUMN reply_to_json jsonb;
ALTER TABLE communication_inputs ADD CONSTRAINT communication_inputs_reply_to_object
    CHECK (reply_to_json IS NULL OR jsonb_typeof(reply_to_json) = 'object');

CREATE INDEX communication_message_sends_input_history_idx
    ON communication_message_sends(scope_id,instance_id,conversation_id,input_id,created_at DESC)
    WHERE input_id IS NOT NULL;
