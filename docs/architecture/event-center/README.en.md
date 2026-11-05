# Event center

[中文](README.md)

The event center is available at `/settings/events`. Definitions, workflow trigger configuration and communication command bindings are durable. Event instances, queues, lifecycle signals and activity observations belong to the current host process and may be lost on restart. Consumers retain ownership of their existing business records.

## Trigger phases

| Phase | Meaning |
| --- | --- |
| `subscribed` | A new effective logical subscription is activated. Restoring configuration, unchanged saves and additional workers for the same subscription do not create new subscription facts. |
| `published` | A valid event is published and admission is attempted independently for each current business consumer. |
| `consumed` | A business delivery is dequeued and ownership is transferred to its consumer, regardless of its eventual business outcome. |

Independent subscriptions have separate queues. Workers within one subscription compete for deliveries. Lifecycle observers receive control signals; receiving one never recursively emits another consumption fact for the original event. A consumption observer alone does not manufacture a business consumption.

An `EventDelivery` continues to hold its resource charge until its guard is released. Dequeue counts, retained resources and successful business execution are separate concepts. Inspection reads snapshots without consuming messages.

## Configuration flow

1. Create a `custom.<name>` contract with a version and payload schema. System and plugin declarations are read-only; only implemented source phases can be selected.
2. Create a Workflow application with the Event trigger. Configure its event, phase and input mapping, then publish it. Missing publication, authority or source availability prevents activation and is reported on the current queues page.
3. Map input names to RFC6901 pointers such as `/payload/order_id`. Existing Start string/number/boolean rules apply. Only explicitly mapped inputs and necessary event identities are included in workflow run records, not an extra copy of the entire event context.
4. Communication bindings support `user/<name>` commands. Map `/arguments`, `/input_id`, `/conversation_id` and `/instance_id` into event fields. Names must not conflict with builtin, plugin or other user commands, including `/new`, `/model` and `/help`.
5. Use existing workflow notification actions to send messages. A command acceptance reply does not confirm workflow completion or notification delivery.

Publication reports admission per subscription. Zero targets is valid but does not retain events for future subscribers. Partial rejection does not roll back other targets or automatically retry all targets. A consumption fact and admission of its observer workflow are separate outcomes. An uncertain prior command attempt is not blindly published again.

## Host resource policy

`API_EVENT_CENTER_POLICY` accepts an explicit JSON host policy. There is no guessed production capacity default. Without a policy, configuration remains available but the runtime is unavailable; the page reports that condition. Restart the host after changing the policy.

| Field | Budget |
| --- | --- |
| `bus_limits.max_items` | Reserved, buffered and in-flight deliveries combined |
| `bus_limits.max_json_bytes` | Combined upper bound of serialized JSON bytes, not process RSS |
| `bus_limits.max_waiters` | Waiting operations |
| `bus_limits.max_topics` / `max_consumers` | Queue generations and consumer owners |
| `bus_limits.max_topic_name_bytes` | Bytes retained by queue directory names |
| `observation_max_bytes` | Recent observation metadata bytes |
| `observation_ttl_ms` | Observation window in milliseconds |

Set values for the deployment's resources. Small fixture budgets exercise boundary conditions and are not product limits. Snapshots identify the host instance; a local value is not a multi-host total. Observations retain metadata without copying event payloads.

## Ownership and authorization

The backend owns the `system.events` Settings registration and authorizes each HTTP method/route independently. Services and repositories enforce workspace isolation. Catalog access does not grant create, update or runtime-inspection access.

The service reuses the host EventBus; workflows reuse the existing publication and execution engine; communication plugins retain provider-specific Receive/Send duties. Existing durable Outbox and diagnostic stream contracts are unchanged.

Use `node scripts/node/tooling.js settings-registry register <descriptor.json>` to edit the canonical Core registration and verify the compiled inventory during acceptance. Descriptors contain `feature_id`, `console_surface` and `api_routes`; the tool does not introduce another runtime permission map.

The only new tables store configuration: `event_definitions` belongs to the `system.events` selective-backup feature and `workflow_event_triggers` to `system.applications`. Existing restore dependency checks apply. Selecting application configuration does not silently copy event definitions owned by another feature. Builtin metadata protects structure and generic record writes; dedicated services own configuration changes.

## Verification

Implementation evidence comes from event-service unit tests, the `event_center_configuration` PostgreSQL target, event routes in `api_system_settings`, event workflow/communication cases in `api_private_contracts`, frontend feature tests and page-debug evidence. Finite design models do not replace implementation tests.

This mechanism does not promise replay, restart recovery, cross-host broadcast or exactly-once external effects. Those capabilities require a separate scope decision; observation logs must not silently become a recovery store.
