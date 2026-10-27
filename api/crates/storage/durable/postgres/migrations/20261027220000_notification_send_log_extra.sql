-- Preserve historic metadata-only logs; do not infer missing message contents.
ALTER TABLE notification_send_logs ADD COLUMN extra jsonb NOT NULL DEFAULT '{}'::jsonb
    CHECK (jsonb_typeof(extra) = 'object');
INSERT INTO model_fields(id,data_model_id,scope_id,code,title,physical_column_name,field_kind,is_system,is_writable,is_required,api_required,is_unique,display_interface,display_options,sort_order,availability_status)
SELECT gen_random_uuid(),m.id,m.scope_id,'extra','Additional information','extra','json',true,false,false,false,false,null,'{}'::jsonb,14,'available'
FROM model_definitions m WHERE m.code='notification_send_logs' AND m.scope_kind='system'
ON CONFLICT DO NOTHING;
