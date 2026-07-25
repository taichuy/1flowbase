---
created_at: 2026-07-23 19
updated_at: 2026-07-25 15
last_verified_at: 2026-07-25 15
memory_type: project
decision_policy: verify_before_decision
status: discussion
source_issue: "#1440"
scope:
  - api/apps/api-server/src/routes/application_public_api
  - api/crates/control-plane/src/application_public_api
  - api/crates/orchestration-runtime
  - api/crates/plugin-framework
  - scripts/node/ai-gateway-concurrency
  - 1flowbase-official-plugins/runtime-extensions/model-providers
---

# Compatible Stream And Gateway Transport Conformance Root 1440

## 2026-07-25 协议门禁边界与 cycle31 停止点

- 用户确认 Claude Code、Codex、OpenCode 不下载、不追最新、不进入 Blocking contract；本机现有版本只作非阻断诊断。
- Gateway Blocking 只证明 Anthropic Messages、OpenAI Chat Completions、OpenAI Responses 的 wire contract、真实 SSE 增量、工具 call/result 关联、显式 terminal/error、完整多轮不复用旧 id/answer/usage，以及 WireAudit no-executor/no-network。
- frozen assembly 为 host `006e0780100cd5c39c776361c83ebbf2c323c06d`、official plugins `40b155590d1d50fbbd9ae22b26cb0a25ce2901d7`。
- cycle31 的 Node 73/73、control-plane 304/304、api-server 196/196 全绿；唯一真实协议门禁执行一次后 RED。HTTP mock scenario header 不会穿过 Gateway，c32 负载超时，Anthropic list correlation 陈旧；WireAudit、secret、cleanup green，客户端未运行且仅记非阻断未验证。
- 同一 conformance fixture 控制边界连续两轮失败，命中 Root 停止条件；#1440/#1452 已返回 `phase:discussion`，不启动 cycle32。下一步需把 Blocking correctness 与 load/observability 分开，并重新确认 mock 场景控制只使用合法协议数据、不增加 Gateway 测试专用透传。

## 2026-07-25 当前检查点

- 用户将真实客户端验收范围固定为 Claude Code、Codex、OpenCode 三项；AionUI 本地 `taichuy_v2_dev` 的 Native GPT Codex / 直连 app-server 增量不进入 #1440 门禁。
- AionCore 与 AionUi 只以 local `main` 作为 ACP 拓扑参考；已分别创建 detached worktree：`AionCore-main@c0b50baf`、`AionUi-main@a5a8b34c7`，原开发工作树及未提交修改保持不动。
- 用户要求把当前一次性本地客户端验收演进为可长期运行的 GitHub Actions 质量门禁。当前方向仍在 discussion：需要对齐 portable client lock、ACP 事件门禁、WireAudit、blocking pinned lane 与 upstream canary 的边界后再实现。
- 1flowbase assembly 仍冻结在 `0f69c382a`，最终 fresh QA、protected branch 合入与 push 继续暂停；新增门禁若获批准，需作为 #1440 新 Delivery/Work Packet 装配后再进行唯一集中 QA。

- 谁在做什么：Root #1440 因 durable transport source-of-truth 决策返回 `grade:g3 / phase:discussion`。D4 WP-D4-01A 与 D3 WP-D3-02 已分别装配为 `5589ebf4f`、`9b6a96a0a`；WP-D4-01B 为 `BLOCKED / NEEDS_SPLIT` 且 diff 为空。D4 #1446、D5 #1447、D6 #1448 暂停，D3 #1443 为 `phase:ready`；Root Control Ledger 是唯一计划真值。
- 为什么这样做：QA cycle 3 证明实时状态机已经事件驱动；Codex 的合法 `web_search` 失败源于 function-only gateway 转换。工具类型应作为传输、流式、关联和终态测试向量，而不是授权 gateway 执行客户端或 Provider 专属工具。
- 为什么要做：gateway 的长期职责是 transparent passthrough、有限 semantic mapping、provider pin、callback correlation、credential redaction 和 terminal integrity。把 web/code/shell/MCP execution、策略 UI、OAuth/SSRF/计费放进 #1440 会错置复杂度和产品 owner。
- 已批准架构：Responses request → GatewayTransportPlan → `TransparentResponses`（native provider、known/unknown opaque passthrough）或 `SemanticCompatible`（仅证明等价的跨协议 subset）→ Provider → RuntimeEventStream → passthrough/mapper → client。
- 执行 owner：caller tools 由 Claude Code/Codex/OpenCode 等客户端执行；hosted/MCP list/call 由 Provider 执行；MCP approval 在 client 与 Provider 间关联；gateway executor invocation 必须为 0。1flowbase application-owned tool 仍只走 existing application runtime。
- 新阻塞：request 创建 run 后写入 durable `flow_run.input_payload`；后续 blocking/streaming execution 只携带 `application_id + flow_run_id`，runtime 按 ID 重新加载 flow run，continuation 也完全从 durable `input_payload` 重建。因此“opaque raw 只在内存 transport 生命周期存在”不能同时满足现有异步、retry/resume 和单一 durable run source of truth。
- 推荐安全边界：在既有 `flow_run.input_payload` 保存版本化 sealed/encrypted Responses transport envelope，不新增表或第二 ledger；plaintext canonical digest 用于 idempotency，key version/nonce/ciphertext 用于恢复，logs/audit 只保存 type/digest/redacted view，retention 跟随 flow run。用户确认前不继续实现。
- 依赖顺序：`#1441 -> #1442 -> #1446 -> #1447 -> #1448 -> #1443`。D4/D5/D6 对 transport/mapper/provider 热点串行；D3 harness 可提前准备，最终只对 frozen paired assembly 执行一个集中 QA。
- 当前基线：protected `dev@d231f7e19c5794855c7c6ff3ce6cfc6415899ed9`；assembly `9b6a96a0ac035a4a7f226bbaedcd1a74709960ad`；official plugins `0cde57f1359836a4f5b999365f56244d085853f4`。没有运行 Cargo、tests、QA、真实客户端或服务；QA3 证据目录仍为 `tmp/test-governance/compatible-stream-e2e/qa-cycle3-20260723-233132/`。
- 保持不变的语义：paired stale history + 新文本 Start 新 run；真正孤立 output 400；exact replay 幂等、conflict 409；RuntimeEventStream 无 DB polling；一个 HTTP turn 单 terminal。
- 截止日期：未设置时间截止；完成条件仅以 Root 28 个 AC、唯一集中 QA、paired integration 与用户验收为准。
- 决策动机：完整工具 inventory 的目的，是证明网关不会丢字段、乱转换、缓冲流或恢复错误 callback；不是把 1flowbase gateway 扩张为通用 Agent Tool Runtime。
