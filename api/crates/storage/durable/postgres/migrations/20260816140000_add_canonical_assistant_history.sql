alter table application_run_conversation_message_items
    add column native_message jsonb;

alter table application_run_conversation_message_items
    add constraint application_run_conversation_message_items_native_message_object
    check (native_message is null or jsonb_typeof(native_message) = 'object');
