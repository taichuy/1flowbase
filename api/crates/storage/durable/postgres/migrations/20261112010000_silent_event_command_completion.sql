-- A Host event command may finish without sending a chat receipt. SQL NULL is
-- unfinished; JSON null is a terminal command with no outbound message.
ALTER TABLE communication_command_events
    DROP CONSTRAINT communication_command_events_check1;

ALTER TABLE communication_command_events
    ADD CONSTRAINT communication_command_completion_state CHECK (
        CASE
            WHEN reply_json IS NULL THEN
                response_session_id IS NULL AND send_status = 'none'
            WHEN reply_json = 'null'::jsonb THEN
                response_session_id IS NOT NULL
                AND send_status = 'none'
                AND outbound_message_id IS NULL
                AND installation_id IS NULL
                AND COALESCE(request_json->>'command_id' LIKE 'user/%', false)
            ELSE
                response_session_id IS NOT NULL AND send_status <> 'none'
        END
    );
