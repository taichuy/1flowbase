---
memory_type: project
topic: runtime-event-stream-fast-debug-plan-approved
summary: RuntimeEventStream 已收敛为 ephemeral 实时哨兵：SSE 等待 typed Closed 后只做一次 durable reconcile；普通 delta 以 64 KiB/20 ms/terminal 批量写历史，终态状态、audit、billing 保持 durable。
keywords:
  - runtime-event-stream
  - first-token
  - debug-stream
  - sse
  - local-runtime-event-stream
  - reasoning-delta
  - agent-thought
  - redis-streams
  - host-extension
  - db-polling
match_when:
  - 讨论 Agent Flow Debug Console、调试流、首 token、SSE、运行事件或 DB polling 性能
  - 设计 RuntimeEventStream、LocalRuntimeEventStream、Redis Streams provider 或缓存类 HostExtension
  - 执行 docs/superpowers/plans/2026-05-02-runtime-event-stream-fast-debug.md
created_at: 2026-05-02 08
updated_at: 2026-07-17 08
last_verified_at: 2026-07-17 08
decision_policy: verify_before_decision
scope:
  - api/crates/control-plane
  - api/apps/api-server
  - web/app/src/features/agent-flow
  - docs/superpowers/specs/2026-05-02-runtime-event-stream-fast-debug-design.md
  - docs/superpowers/plans/2026-05-02-runtime-event-stream-fast-debug.md
---

# 调试流首 token 加速方案已确认

## 时间

`2026-05-02 08`

## 谁在做什么

用户与 AI 已确认 `RuntimeEventStream 与调试流首 token 加速设计`，并批准按 `docs/superpowers/plans/2026-05-02-runtime-event-stream-fast-debug.md` 执行本地单 API server 阶段。AI 负责持续推进实现、验证，并在每个计划任务完成后同步更新计划文档。

## 为什么这样做

当前 Agent Flow Debug Console 的流式调试相对 Dify 预览慢，核心瓶颈是 SSE 返回前等待同步启动流程、节点状态依赖高频 DB polling、provider delta 与持久化压力没有充分解耦。目标是让 HTTP stream 和执行任务通过宿主运行事件流解耦，让首个可见响应更快出现。

## 为什么要做

Dify 预览体验的关键不是所有状态都先落库，而是先建立实时输出通道，再把可追溯事件异步持久化。1flowbase 需要在保证 PostgreSQL 仍是 durable truth 的前提下，把 live delta、node lifecycle、persister、metrics/tracing 拆成不同消费者，减少首 token 前等待和 100ms DB polling 压力。

## 已确认决策

- 新增宿主拥有的 `RuntimeEventStream`，它是按 run 维度组织的短期有序运行事件流，不是普通 key/value cache，也不是通用业务事件总线。
- 第一阶段用进程内 broadcast + ring buffer 实现 `LocalRuntimeEventStream`，适合本地调试和单 API server，默认不要求外部缓存。
- 第二阶段 Redis Streams、NATS JetStream、Kafka 或等价队列只能作为 HostExtension provider 实现同一合同；Core 只依赖 `RuntimeEventStream`，不直连 Redis。
- 调试 SSE 认证后 fast-start，先返回 `flow_accepted` 或 heartbeat，再后台 compile / execute。
- provider live delta 优先 append 到 `RuntimeEventStream` 并推给 SSE；DB event/span/audit 由异步 persister 后写。
- node lifecycle 由 runtime 直接 append `node_started/node_finished/node_failed`，不再把 100ms DB polling 作为实时来源。
- audit/billing required 事件不能只停留在易失 ring；PostgreSQL 仍是 durable truth，持久化失败要可诊断但不阻塞普通 SSE token 输出。
- 前端按 event type 分离 message delta、trace 和 variable cache 更新；text delta 不应每 token 重建 variable cache。
- `reasoning_delta` / thought 是模型生成的一部分，不应被过滤掉；调试流应参考 Dify 的 `agent_thought` 独立区域，把思考内容单独展示，同时像 `text_delta` 一样进入 RuntimeEventStream、短期缓存和 durable debug event 持久化。
- 复制正式输出默认只复制 answer/content，不把 reasoning 混入最终答案；reasoning 作为独立生成内容可单独展示和恢复。
- 执行方式上，同一时间只运行一个独立 agent / 子 agent，防止系统资源不足；每完成一个任务必须更新计划状态。
- 2026-05-08 补充确认：Agent Flow Debug Stream 历史恢复以 `runtime_events` 为唯一真相源，`flow_run_events` 不再作为新调试流正文或历史恢复来源。
- 2026-05-08 补充确认：`RuntimeEventStream` 只是运行期 best-effort 传输和 SSE source，进程内缓存丢失可接受，能写多少写多少；长期缓存/中间件能力后续由 HostExtension provider 扩展，不在技术底座内继续补强。
- 2026-05-08 补充确认：该能力仍处于开发初期，不为旧 debug stream 逻辑保留兼容 merge path。

## 截止日期

无固定截止日期；本地单机阶段已在 2026-05-08 按 `docs/specs/2026-05-08-agent-flow-debug-stream-runtime-events-spec.md` 与 `docs/plans/2026-05-08-agent-flow-debug-stream-runtime-events-plan.md` 完成一次 debug stream 历史真值收敛。进入 Redis Streams 或等价外部 provider 属于后续 HostExtension provider 扩展阶段，不在当前技术底座实现计划内。

## 2026-07-17 运行时降温收敛

- `RuntimeEventSubscription` 暴露 typed `RuntimeEventClosure { reason, final_sequence }`；晚订阅者也能读取关闭快照。
- Native / compatible SSE 已取消固定 500 ms durable polling。事件流保持打开时不查终态；receiver 关闭后先检查 typed closure，只做一次 durable runtime-record reconcile，必要时再做一次 durable run-state fallback。
- compatible resume 不再按 answer chunk 查询 PostgreSQL。live chunk 在内存中按 Answer Presentation identity 累积；带 `event_ids` 的 durable 合并记录只补缺失后缀，普通 live chunk 不按前缀裁剪。
- `LocalRuntimeEventStream` 同时受 `max_events` 与 `max_bytes` 约束；默认 16 MiB。内存压力下先淘汰 `Ephemeral`，`DurableRequired / AuditRequired` 才保留并反压。
- text / reasoning / Answer Presentation delta 使用 `Ephemeral + persist_required=true`：历史 persister 会尝试写入，但进程崩溃或队列饱和时允许丢失尚未 flush 的短窗口。
- runtime history 采用 64 KiB、20 ms、terminal / receiver-close 三重 flush；PostgreSQL 使用既有 multi-row append。Answer Presentation provider 转发使用容量 64 的有界队列，饱和或写失败后停止后续历史写入以保持 durable prefix，terminal suffix 负责恢复最终答案。
- 合并 delta 必须保留 `node_id` 与完整 `presentation` identity；不同 segment/source 不合并。provider forwarding task 在返回执行结果前等待批量 persister，作为 terminal barrier，保证 callback / final suffix 查询不会越过未 flush 前缀。
- lifecycle / terminal 已成功落库后，其 RuntimeEventStream 投递副本降级为 `Ephemeral + persist_required=false`，避免 16 MiB ring 被已 durable 的大 payload 填成全不可淘汰，也避免 debug persister 重复写入。
- `flow_run.status`、audit、billing 继续走原 durable 路径；Redis 仍只作为未来 `RuntimeEventStream` HostExtension provider，不进入 Core 直连依赖。
- 任务级验证覆盖 LocalRuntimeEventStream、Native/compatible SSE 无轮询与关闭补偿、compatible protocol projection、runtime-event batch、callback resume exactly-once。既有 NUL callback fixture 在未改动 HEAD 同样因 `invalid provider_code` 留下 `Pending`，记录为本任务外既有测试问题。

## 决策背后动机

用户认可“先响应、旁听放缓存/事件流、再持久化”的方向，但要求这个能力不能被设计成主仓库内置缓存依赖。正确边界是宿主 runtime event stream 合同：本地默认轻量实现，外部缓存/队列由 HostExtension provider 接入，既保留 Dify 类快预览体验，也不破坏 1flowbase 单机默认无外部缓存的产品边界。

## 关联文档

- [Runtime Event Stream 与调试流首 token 加速设计](/home/taichu/git/1flowbase/docs/superpowers/specs/2026-05-02-runtime-event-stream-fast-debug-design.md)
- [Runtime Event Stream Fast Debug Plan](/home/taichu/git/1flowbase/docs/superpowers/plans/2026-05-02-runtime-event-stream-fast-debug.md)
