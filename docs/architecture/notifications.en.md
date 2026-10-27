# Notification instances and plugin sends

Notification types, workspace instances and recipient inbox messages have separate owners. Settings stores instance configuration; a workflow notification node selects an instance and supplies invocation input. The Host provides only the built-in `system` type. Managed plugins contribute other types.

## Persistence and delivery

`notification_instances` stores workspace identity, type, installation, display name, status, public configuration and encrypted secrets. Password controls identify secret string fields. Read responses omit secret values and return only `configured_secret_fields`; updates retain omitted secrets.

`system_notifications` stores one message and independent `read_at` per recipient. Every recipient must be an active member of the current workspace. The transaction commits all recipients before publishing a refresh signal. Inbox reads, unread counts and read operations filter by workspace and authenticated recipient. Deleting an instance or disabling/uninstalling a plugin preserves messages. Sending through a missing, disabled or unavailable instance fails explicitly.

A system send succeeds when the message transaction commits. An external result represents the provider's declared receipt confirmation, not a user read. The credentialed `notification.changed` SSE carries refresh signals only. Opening or reconnecting the stream refetches the HTTP/database snapshot; the stream is not a durable message queue.

## Authoring a type

The [complete fixture manifest](../../api/plugins/fixtures/acme.notification/manifest.yaml) declares `notification_types`. Each descriptor has a namespaced `type_code` (`publisher_namespace/plugin_id/local_type`), `display_name`, `description`, `contribution_id`, separate `config_schema`/`config_form`, `input_schema`/`input_form`, and an object `output_schema`. The Host reserves `system`.

Forms use the existing `PluginFormSchema` version `1.0.0` and string, number, boolean or select controls, defaults and conditions. `password` marks configuration string fields; `multi-select` represents an array and validates each option against `items`. Schemas are self-contained and cannot resolve external references. The built-in system input form receives current-workspace member options from the types API.

Bind a contribution to `1flowbase.notification.send`, version `1flowbase.notification/v1`, requiring `notification.send`, with binding payload `{kind: notification_type, type_code: ...}`. Installation, workspace assignment, independent contribution grants and activation follow the existing managed lifecycle. A declaration does not grant execution. Grant `permission_contract_id: notification`, `permission_contract_version: 1`, with workspace resource scope.

## Worker execution

The existing capability envelope carries `method: execute` with `input.config_payload` and `input.input_payload`. The shared Host owns instance policy, trusted actor/workspace, frozen contribution admission, authority checks, process dispatch and lifecycle. The plugin owns provider delivery and its declared result. It needs no duplicate Host transport or database connection.

Rust workers can call `runtime_extension_sdk::execute_notification`. The [SDK worker](../../api/crates/runtime-extension-sdk/src/_tests/notification_worker.rs) demonstrates one-shot stdio. A multiplex worker uses the existing `serve` handler, deserializes `CapabilityStdioRequest`, invokes the same helper and serializes the response. This release supports process-per-call stdio and multiplex; it does not promise an actor service pool for notification types.

Build `cargo build --locked --manifest-path api/Cargo.toml -p runtime-extension-sdk --example notification_worker`, resolve the executable from the configured Cargo target directory, then package `api/plugins/fixtures/acme.notification` through `scripts/node/plugin/cli.js package --runtime-binary <absolute executable>`. The fixture does not contact an external provider. No SSH/OpenAI plugin changes or publication are required.

## Workflow and HTTP

The native node has `type: notification` and `config.instance_id`. `bindings.input` uses existing `named_bindings` entries with descriptor field names, constant/selector/templated_text expressions and `valueType: number` for numeric expressions. The runtime forwards the resolved input object to the shared service. Output is `{instance_id, type_code, result}`; the default result variable uses the selected type's output schema and supports nested downstream selectors.

Published and resumed runs retain their frozen actor/workspace even if the user's current workspace later changes. A single-node preview sends once without replaying upstream nodes. The same service handles workflow and HTTP sends.

`/api/console/notifications` owns type discovery, instance CRUD/send, recipient messages, unread counts, read/read-all, and refresh events. Settings methods have independent console operations; authenticated types and recipient inbox APIs do not require member-management permission. Business HTTP enters the Canonical Interface and typed Handler. Events is a read-only protocol control entry.

Real package intake, PostgreSQL, independent grants and SDK binary fixtures cover sends, schema failures, secret redaction and retained unavailable instances. Actual acceptance evidence is recorded under the task's `tmp/test-governance/`; examples and source fixtures alone do not constitute a test pass.

## Send logs and personal channels

Every authorized workspace send persists one pending `notification_send_logs` fact before validation or execution. The shared `NotificationSenderPort` receives a trusted `api` or `workflow` source from its caller; HTTP bodies cannot select it. Instance and actor UUIDs have no foreign keys and historical sends are not backfilled. Missing instance names/types are nullable. Log list/get have independent workspace-scoped settings operations.

System messages and log success still commit in one transaction with a deduplicated recipient count. Types declaring `log` persist validated, credential-checked title, full content and input extra fields before provider I/O. Completion merges only declared business receipt fields into the `extra` JSONB object. Whole raw inputs/results, config, managed credentials, runtime envelopes and raw errors are never logged. Pre-send instance/type/plugin/input/config/recipient rejection has a fixed failure code. Provider errors may follow an external effect and become unknown/provider_result_unknown. Unsafe or schema-invalid provider output becomes unknown/provider_output_invalid. Failed finalization leaves pending rather than asserting definite failure.

The channels endpoint aggregates only the current workspace and authenticated recipient. It applies all/unread/read before choosing each channel's latest message, orders by that latest time, and independently counts all unread messages per channel. Cursor pages are bounded to 100. Messages support instance_id/read_status; explicit read_status takes precedence over the retained unread_only parameter. LEFT JOIN preserves deleted-instance history with deleted=true, display_name='Deleted notification channel', type_code='system', because the inbox table contains only system messages.

Message unread clears only the recipient's read_at. Channel read-all changes only that recipient/channel, including deleted-channel history. Both publish recipient/workspace refresh signals. Existing global read-all remains available.

## Plugin-declared send logs

```yaml
log:
  content: /text
  extra:
    accepted: {source: result, pointer: /accepted}
    delivery_id: {source: result, pointer: /delivery_id}
```

The optional `title` and required `content` are RFC 6901 JSON Pointers into declared string fields of the business input, including `~0` and `~1` escaping. A missing title mapping, absent title or blank title uses the first 255 Unicode scalar characters of content. The full body and any explicit title remain unchanged. Missing or non-string content is rejected before provider I/O. Each `extra` key declares `source: input|result` and a pointer to a JSON value; absent optional values are omitted. Root pointers, undeclared fields, writeOnly fields and invalid escapes are rejected. Configuration, managed secrets and runtime envelopes are not available as mapping sources.

The Host checks the complete message projection against known password configuration and managed credentials. Unsafe input projections are rejected before delivery and are not stored. Results pass their business schema and existing credential checks before receipt projection. Failed or uncertain attempts retain their previously persisted message. The UI consumes the Host-generated title and complete body and renders extra fields as structured key/value data. Legacy types without `log` remain loadable and keep their metadata-only behavior; missing historic content is not inferred, and existing rows receive `extra: {}`.

## Notification instance QR authorization

The optional descriptor `authorization` declares QR actions and `managed_secret_keys`.
Its absence preserves the original send contract. Authorization uses the same
`1flowbase.notification.send` contribution, Capability Execute envelope and managed
dispatcher; it creates no model provider or notification slot. Rust suppliers reuse
async `execute_authorizable_notification` inside the shared SDK `serve`. The
authorization fixture uses `stdio_json_multiplex_v1` at both package and binding
runtime levels; the legacy one-shot fixture keeps `stdio_json`.

`POST /api/console/notifications/instances/{instance_id}/authorization` accepts the
canonical `NotificationAuthorizationOperation`: `{type:"begin",action:"qr"}`,
`{type:"poll"}`, `{type:"cancel"}`, or `{type:"maintain"}`. This mutating endpoint
requires CSRF and the independent `notifications.instances.authorization` console
operation. Canonical Interface and its typed handler dispatch to the service,
which checks workspace and permission again. Historical settings grants do not
implicitly receive this new operation; root retains existing administrator access.

ApiSuccess data and the instance DTO's `authorization` use the same
`NotificationAuthorizationView`, with `status=unauthorized|pending|authorized|invalid`.
Pending requires `user_action={kind:"qr_code",qr_code_url,poll_interval_seconds,expires_at?}`.
Only a reasonable HTTPS display URL is public; internal QR challenges, bot/context
tokens, recipient binding and provider errors are excluded. Legacy authorization
is null. Administrator `status=enabled|disabled` remains independent; successful
authorization never enables a disabled instance.

The Host stores the public view in `notification_instances.authorization_json`
and encrypts managed credentials in `encrypted_managed_secret_json`, separately
from user-editable password configuration in `encrypted_secret_json`. Both use
workspace/instance identity and distinct AAD. The existing selective-backup rekey
path handles both ciphertext domains so a restore under a new master key retains
authorization. Managed keys are forbidden in public/editable config. Patches may
contain declared keys only; null deletes a key, omission preserves it. A supplier may
encrypt/cache displayable `qr_code_url`; only that exact authorization user-action
display field is exempt from reflection checks. The same cached value in send
result, expiry or another field is still rejected.

Persisted `authorization_revision` fences state writes by CAS. Begin/cancel claim
a new revision and remove old managed credentials before provider I/O, without
holding a database transaction lock during the network call. Poll atomically
writes the public view and encrypted secrets only if that revision is unchanged.
Delayed poll results cannot overwrite cancellation or a fresh begin. Old send
patches likewise cannot replace newer authorization; discarding an obsolete patch
does not turn an externally confirmed send into a failure. A reconstructed Host
recovers pending challenges and authorized credentials from durable storage,
without depending on worker memory.

Execute input_payload is the canonical `NotificationRuntimeInput`, with authorize
or send `operation` and Host-injected `managed_secrets`; config_payload stays
separate. `NotificationRuntimeResult` separates public authorization from the
Host-only secret patch. The Host independently validates response correlation,
allowlists, output schemas and credential reflection even when a worker bypasses
SDK helpers. Invalid patches are never persisted and raw results are never logged.

Unauthorized/pending sends are rejected locally as failure/authorization_required;
invalid authorization produces failure/authorization_invalid. Authorized accounts
may attempt provider sending without a context token or recipient-ready boolean.
An explicit recipient_not_ready provider response preserves account authorization;
a definite provider_rejected response is failure. Provider errors, invalid responses,
credential reflection or ambiguous external effects remain unknown/provider_result_unknown.
Valid cursor/context patches may accompany success, rejection or unknown outcomes
and are encrypted by the Host. A real supplier matches inbound context only to the
scanned bound user's peer; another sender cannot rebind the recipient.

The API test fixture `extension_bus/notifications/authorization.rs` uses the built
SDK `notification_authorization_worker` at the absolute path in
`NOTIFICATION_AUTHORIZATION_WORKER_FIXTURE`. It covers real package intake,
independent contribution authority, canonical HTTP, encrypted PostgreSQL, Host
reconstruction, controlled stale polling, fixed failure logs and legacy system
notifications. It is not evidence of real WeChat delivery. Actual execution
receipts belong to the Root's concentrated QA batch.
