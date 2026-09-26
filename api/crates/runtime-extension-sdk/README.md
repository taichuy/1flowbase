# 受管插件作者 SDK

SDK 提供 typed Hook / event worker 协议与作者侧 client。Host 负责可信 context、权限、调用 scope 和进程生命周期；SDK 不提供 SQL、数据库连接、宿主 registry 或无限制 RPC。plugin → Host 回调不能自行声明可信 installation / workspace / actor，Host → worker 的 typed context 由 Host 注入。

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

`cancel` aborts and awaits the call task before emitting `cancelled`. If a
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
