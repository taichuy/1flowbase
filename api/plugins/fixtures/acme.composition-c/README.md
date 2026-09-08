# Composition C event subscriber

This fixture declares its own `processed_models(model_id,status,result_reference)` collection.
Package `manifest.yaml` with the prebuilt `runtime-extension-sdk --example managed_event_worker`
binary at `bin/worker.py` (the archive entry is retained for the shared fixture packager).
Its `apply_processed` handler requests only the received processed payload. The host chooses
the owner, workspace, table and stable event/subscriber receipt identity.

Grant `event.subscribe` (`managed-event@1`, workspace) and `plugin_data.owned.write`
(`plugin-data@1`, own `processed_models`) to `.events`, plus the separately declared `.data`
write grant. A `.data` grant alone cannot authorize the subscriber effect.
Apply the manifest through the extension center schema owner before enabling the subscriber.
ACK follows the receipt/effect transaction commit. See the Root AC-007 API fixture.
