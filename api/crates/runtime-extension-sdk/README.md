# 受管插件作者 SDK

SDK 提供 typed Hook / event worker 协议与作者侧 client。Host 负责可信 context、权限、调用 scope 和进程生命周期；SDK 不提供 SQL、数据库连接、宿主 registry 或无限制 RPC。plugin → Host 回调不能自行声明可信 installation / workspace / actor，Host → worker 的 typed context 由 Host 注入。

通讯贡献使用公共内容参数生成 Markdown：普通文本、结构化段落/列表和已有 Markdown 有明确区别，底座与 SDK 复用同一 renderer。规范与示例：[中文](docs/communication-messages.md) / [English](docs/communication-messages.en.md)。

当前示例覆盖只读 Create Hook 与 committed / processed 事件。贡献必须经正式包安装、工作区分配、独立授权与执行绑定；manifest 声明不是授权。同包贡献不共享权限。事件消费的 owned-write 必须授予实际消费贡献。

## 构建实际 worker

在仓库根目录、Linux 上执行：

```bash
export CARGO_TARGET_DIR="$(pwd)/tmp/quality-gate-cache/plugin-composition-2007/target"
cargo build --locked --manifest-path api/Cargo.toml -p runtime-extension-sdk --example managed_hook_worker
cargo build --locked --manifest-path api/Cargo.toml -p runtime-extension-sdk --example managed_event_worker
export MANAGED_HOOK_WORKER_FIXTURE="$CARGO_TARGET_DIR/debug/examples/managed_hook_worker"
export MANAGED_EVENT_WORKER_FIXTURE="$CARGO_TARGET_DIR/debug/examples/managed_event_worker"
```

这两个 example 来自 `src/_tests/managed_hook_worker.rs` 与 `managed_event_worker.rs`。验收使用绝对路径和真实构建产物，缺少环境变量不能静默跳过。示例是有限 fixture，不是任意接口的自动开放承诺。

## 正式包

```bash
node scripts/node/plugin/cli.js package api/plugins/fixtures/acme.composition-a \
  --out tmp/plugin-composition-packages \
  --runtime-binary "$MANAGED_HOOK_WORKER_FIXTURE" --target x86_64-unknown-linux-gnu
```

CLI 将可执行文件放入 manifest 指定的 `bin/worker.py`；这个历史文件名不代表 Python，正式复现使用上面的 SDK ELF 二进制。事件变体及 B/C 见 [A](../../plugins/fixtures/acme.composition-a/README.md)、[B](../../plugins/fixtures/acme.composition-b/README.md)、[C](../../plugins/fixtures/acme.composition-c/README.md)。

授权、幂等限制、epochs、退休、资源预算与关闭顺序见[插件组合架构](../../../docs/architecture/plugin-composition.md)。当前新增源码 fixture 尚待 Root 集中 QA，构建与打包命令是复现说明，不是已执行证据。

## Tokio multiplex worker

A package that declares `runtime.protocol: stdio_json_multiplex_v1` may run one
long-lived worker with overlapping calls. The canonical NDJSON envelope is in
`extension-contracts::stdio_multiplex`. The Host must assign canonical
positive decimal `u64` call IDs in strictly increasing order at actual writer
dispatch for each worker incarnation, and fence responses to that process instance. The runner retains
only the highest dispatched ID; an old cancel is a no-op, while duplicate or
out-of-order calls fail closed. No finished-ID cache grows with uptime.

```rust,ignore
runtime_extension_sdk::serve(|request, emitter| async move {
    // Deserialize `request` to the relevant typed ProviderStdioRequest,
    // DataSource request, etc. The handler may serialize by session.
    let _ = emitter.try_event(serde_json::json!({"type": "text_delta", "delta": "hello"}));
    // Convert both success and domain errors into the slot's existing typed
    // response envelope, then serialize that envelope as the terminal Value.
    serde_json::to_value(my_typed_response).expect("typed response is serializable")
}).await?;
```

`serve` accepts `Fn(Value, MultiplexEmitter) -> Fut`, where
`Fut: Future<Output = serde_json::Value> + 'static`; neither the handler nor
future requires `Send`. Run it inside a Tokio runtime. `serve_io` accepts
injectable async input/output and requires a `LocalSet`; it is intended for
fixtures. The handler performs slot-specific request validation and maps its
own `Result` into the pre-existing typed response/error shape. The transport
never invents a business error or unwraps an inner response. An emitter event
uses `try_event` and returns `MultiplexError::Backpressure` if its bounded
output queue is full. A callback uses `callback(PluginDataV1, request).await`
and awaits a result correlated to `call_id` and `callback_id`; the Host resolves
trusted binding itself.

A Host call and worker terminal response look like:

```json
{"protocol":"stdio_json_multiplex_v1","kind":"call","call_id":"42","request":{"method":"invoke","input":{}}}
{"protocol":"stdio_json_multiplex_v1","kind":"response","call_id":"42","response":{"output":{}}}
```

`cancel` closes per-call input, invalidates emitters and closes output credit.
The handler may finish awaited cleanup; after a one-second transport grace the runner
aborts it. `cancelled` follows observed task termination. If a
response already won the race, a late cancel for any dispatched ID is a no-op.
Unknown future controls and duplicate calls fail closed. The canonical
`MULTIPLEX_MAX_FRAME_BYTES` limit is 32 MiB, leaving room around the API's
existing 16 MiB public response limit for typed JSON envelopes. A frame over
1 MiB is valid and covered by a transport fixture.

The SDK uses one shared `MULTIPLEX_OUTPUT_BUDGET_BYTES` (64 MiB) semaphore for
encoded event, callback, terminal, and pending callback-result bytes. Each
output permit remains held through the actual stdout write and flush. The
output queue is bounded by those bytes, not by an arbitrary number of messages.
Synchronous event producers report `MultiplexError::Backpressure` on byte pressure.
Each active call can hold one bounded terminal frame while awaiting the output
budget; its Host resource reservation remains held until terminal acknowledgment.
The completion channel has 16 slots and waits asynchronously; slot pressure does
not terminate the worker. Terminal sending does not block input cancellation.
The Host's per-call event budget is the separate canonical
`MULTIPLEX_CALL_EVENT_BUDGET_BYTES` (32 MiB). These transport safety budgets
are not limits on provider business concurrency. Fixed writer and stdin
bridge buffers are allocated once per worker process, not once per session.

`serve` bridges stdin through a detached reader thread so an open Host pipe
cannot trap Tokio's blocking pool during runner shutdown.

## Managed duplex calls

`serve_duplex` accepts `Fn(Value, MultiplexEmitter, MultiplexInput) -> Fut`;
`serve_duplex_io` provides the same contract for injectable I/O. Existing `serve`
and `serve_io` remain available. `MultiplexInput::recv().await` returns `None`
when Host cancellation closes the call, allowing the plugin to await its own
connection cleanup. During cancellation `emitter.event` fails with `Correlation`.

`emitter.event(value).await` reserves sliding byte credit, queues an Event and
returns without waiting for a network roundtrip. Events contain `await_ack: true`
and a monotonically increasing `event_id`. Host `event_ack` releases that event's
credit after downstream consumption. Multiple events can be in flight;
`MULTIPLEX_CALL_EVENT_BUDGET_BYTES` bounds their aggregate encoded bytes. A terminal
Response waits for all event credit to return. Input consumption and output
production must run concurrently in the handler. `try_event` retains its original
wire shape and fail-fast behavior. Host channels, process ownership, identity,
authority and lifecycle remain outside the plugin.

受管双向调用通过 `serve_duplex` / `serve_duplex_io` 注入每次调用的输入。
取消使 `MultiplexInput::recv` 返回 `None`，使 `event` 返回 `Correlation`；
插件据此等待自身连接清理，SDK 在观察任务结束后发送 `Cancelled`。
输出使用按字节滑动窗口，`event` 排队后即返回，多条输出可同时在途；
宿主确认下游消费后，按 `event_id` 归还 credit。终态等待输出全部消费，
输入处理不等待每条输出的网络确认。宿主独占身份、鉴权和进程生命周期。

## Opt-in notification authorization and send outcomes

Legacy `execute_notification` still consumes raw `input_payload` and returns the
provider's declared output. A notification descriptor opts in by adding
`authorization`; no new slot, provider type, capability method or worker protocol
is introduced. All operations use the installed notification contribution's
existing Capability `execute`, `config_payload`, and managed dispatcher.

```json
{"authorization":{"actions":[{"code":"qr","label":"Scan QR code","kind":"qr_code"}],"managed_secret_keys":["qr_credential","bot_token","bound_user_id","context_token","cursor"]}}
```

The canonical `NotificationRuntimeInput` is the Execute `input.input_payload`:

```json
{"method":"execute","input":{"contribution_code":"send","config_payload":{},"input_payload":{"operation":{"type":"authorize","operation":{"type":"begin","action":"qr"}},"managed_secrets":{}}}}
```

`NotificationAuthorizationOperation` supports `begin`, `poll`, `cancel`, and
`maintain`. `begin` requires a declared action code. For sends, use
`operation: {"type":"send","input_payload":{"text":"hello"}}`; the nested
payload still validates against the descriptor's original `input_schema`.
Only the Host injects `managed_secrets` from encrypted instance storage.
User configuration cannot contain any declared managed key, including when its
schema permits additional properties.

Capability `result` contains `NotificationRuntimeResult`, rather than the legacy
provider output. An authorization response example:

```json
{"ok":true,"result":{"type":"authorization","authorization":{"status":"pending","user_action":{"kind":"qr_code","qr_code_url":"https://example.com/qr","poll_interval_seconds":2,"expires_at":"2026-10-15T01:00:00Z"}},"managed_secret_patch":{"qr_credential":"HOST_ONLY"}}}
```

`NotificationAuthorizationView` is the canonical public DTO: `status` is
`unauthorized | pending | authorized | invalid`, with `user_action` required only
while pending. User action contains only QR display URL, polling interval and
optional expiry. QR URLs require HTTPS and cannot contain URL credentials.
Internal polling credentials, tokens, recipient binding and raw provider errors
have no public field. Clients consume the descriptor's `authorization` and the
instance's separate `authorization` view using these same field names. Their
authorization command body uses `NotificationAuthorizationOperation`; the Host
owns scope/permission enforcement and injects the instance config and secrets.

Send result examples (these are inside Capability `result`):

```json
{"type":"send","outcome":{"status":"succeeded","result":{"accepted":true}},"authorization":{"status":"authorized"},"managed_secret_patch":{"cursor":"next","context_token":"HOST_ONLY"}}
{"type":"send","outcome":{"status":"failed","failure_code":"authorization_invalid"},"authorization":{"status":"invalid"},"managed_secret_patch":{"bot_token":null,"context_token":null}}
{"type":"send","outcome":{"status":"unknown","failure_code":"provider_result_unknown"}}
```

Confirmed failures use only `authorization_required`, `authorization_invalid`,
`recipient_not_ready`, or `provider_rejected`. Uncertain provider delivery uses
`status: unknown` and `provider_result_unknown`; validation rejects putting that
code in `failed`. The Host maps unknown to the existing unknown notification log
status. Authorization failures require the corresponding public view update;
recipient_not_ready preserves account authorization. Optional send authorization
means no status change when omitted. No recipient-ready boolean or context-token
prerequisite is imposed: the supplier can attempt sending without inbound context
and classifies a definite upstream rejection from the actual response.

`managed_secret_patch` appears only in Host/runtime envelopes and accepts declared
keys only. Omitted keys stay unchanged; null deletes a key. Send can persist new
inbound cursor/context even when delivery fails. Provider-specific maintenance
matches only the bound scanned user id; other inbound peers must never rebind or
replace that user's context. These are supplier/Host ownership requirements, not
an alternate SDK polling service.

Use `execute_authorizable_notification(..., |config, input| async move { ... })`
inside the existing `serve`/`serve_duplex` handler. Lower-level
`prepare_authorizable_notification` and `complete_authorizable_notification`
are available for authors needing separate await/lifecycle handling. Preparation errors
use `Box<CapabilityStdioResponse>` to keep the result small; forwarding the original
wire response requires `*response`. The SDK
validates before supplier I/O and redacts callback errors. Errors or invalid
responses after a send become unknown, without exposing unvalidated patches.
The Host must validate the response again; a worker can bypass SDK helpers.

Host persistence guidance for the next implementation packet: keep administrator
`enabled`/`disabled` independent of authorization, store public authorization
state in its own instance projection, and encrypt QR credentials, bot token,
context token, cursor and recipient binding in separate managed-secret storage.
Bind pending attempts to a persisted revision/challenge; discard a delayed poll
response after cancel/re-begin instead of overwriting newer state. Do provider
I/O outside a database transaction lock. Cancel removes the pending challenge.
Cache a displayable QR URL as `qr_code_url` and its encoded challenge as
`qr_credential`. The Host allows those two values only in the exact cached
`user_action.qr_code_url`; reflection into another output field is rejected.
This exception never permits bot tokens or other managed secrets in a QR URL.
Never log raw config/input/result envelopes, QR credentials, tokens or provider errors.
The optional descriptor `log` selects public business input fields into title/content and
input/result fields into extra information. The Host checks credentials before storage,
retains complete content and defaults a missing title to its first 255 Unicode characters.
See the bilingual [notification contract guide](../../../docs/architecture/notifications.en.md#plugin-declared-send-logs).

The `notification_authorization_worker` example under
`src/_tests/notification/authorizable_worker.rs` uses the shared multiplex runner
with deterministic begin/poll/cancel/maintain/send responses and secret patches.
It is a Host persistence fixture, not evidence of real provider delivery. Contract
and SDK test execution is deferred to the Root's one assembled QA batch.

## Application logs and HTTP Host services

`ApplicationLogsClient::new(emitter.clone()).execute(ApplicationLogsRequest)` and
`HttpEgressClient::new(emitter).execute(HttpEgressRequest)` use the same multiplex
callback dispatcher. The canonical types are reexported from `extension-contracts`.
Responses are typed; `HostServiceError` retains a safe `code`, optional HTTP `status`
and structured `details` (including partial import receipts).

These are two fixed Host service families, not route forwarding or arbitrary RPC.
Managed service contributions declare and receive independent system grants:
`application.logs.read` / `application.logs.import` with permission contract
`application-logs` version `1`, and `network.http.request` with `http-egress` version
`1`. The actual caller's session, operation and application/workspace rights are
rechecked by the Host. No worker request supplies actor or installation identity.
System plugin data and credentials keep their own existing system storage scope.

HTTP requests use Host Network Egress with closed consumer kind `managed_plugin`
and `consumer_reference` equal to the installed plugin UUID, in the real caller's
workspace. Redirects are returned unchanged; the plugin interprets remote status
and body. Cancellation/deadline drops release Host leases. Full selected log facts
are returned or an explicit transport-budget error is raised; the 32 MiB frame
budget never silently truncates records. Page complete record lists and export
selected records in separate calls when a larger overall exchange is needed.
