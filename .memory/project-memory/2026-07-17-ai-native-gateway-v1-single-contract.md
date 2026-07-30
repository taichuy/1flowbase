---
memory_type: project
topic: AI Gateway Canonical Stream 单一流式真值
summary: #1453 树已由 #1461 supersede；新 Root 以 Canonical Stream State 统一三协议 SSE、Responses WebSocket、durable 正文与集中质量门禁，当前等待用户审阅后再实施。
keywords:
  - issue 1366
  - issue 1461
  - AI Gateway V1
  - single contract
  - Issue Tree Root
  - Delivery Tree
  - canonical stream
  - responses websocket
created_at: 2026-07-17 22
updated_at: 2026-07-26 09
last_verified_at: 2026-07-26 09
decision_policy: verify_before_decision
scope:
  - https://github.com/taichuy/1flowbase/issues/1461
  - /home/taichuy/git/1flowbase
  - /home/taichuy/git/1flowbase-official-plugins
---

# AI Gateway Canonical Stream Single Truth

## 当前活动真值

- 唯一活动 Root 是 #1461，当前为 `phase:discussion`；用户要求先审阅线上计划，确认前不实现、不运行 QA。
- #1453～#1458 已于 `2026-07-26` 以 superseded 关闭；既有 assembly 不丢弃，冻结 SHA `d2b4779894e2b40a21417931917774d74f4ee7fc` 是 #1461 的 inherited baseline。
- 单一链路固定为：外部协议 → AI Native typed operation → Start system input → AgentFlow → actual LLM consumer → Provider adapter → typed terminal → 外部协议投影。
- 流式响应固定为：Provider deterministic transducer → typed Native stream event → turn-local single-writer Canonical Stream State → 公共协议 transducer / durable terminal transaction。
- 主仓基线：`dev@922f87806b4713c8ef2599214cbf3fedc131b88d`；official plugins 基线：`main@40b155590d1d50fbbd9ae22b26cb0a25ce2901d7`。
- 隔离 assembly：`codex/issue-1453-ai-native-operations`，路径 `/home/taichuy/git/1flowbase_git_workspace/issue-1453-ai-native-operations`。

## Delivery Tree

1. #1462 Canonical Stream State 与三协议 SSE 无损投影。
2. #1463 Responses WebSocket 公共网关与 Canonical turn parity。
3. #1464 工具回调、失败终态与有界背压。
4. #1465 可持续流式协议质量门禁与三客户端集中验收。

依赖为 `#1462 → #1463`，`#1462 → #1464` 且 WebSocket lifecycle rows 依赖 #1463，最后 `#1465` 集中门禁。全部产品与 fixture Packet 装配后只执行一次 Root 集中 QA。

## 执行边界

- Operation Binding 不再是作者配置、发布快照 routing 真值或入口 preflight；若保留 projection，只能由 compiler 派生。
- raw Provider body、secret、continuation/resource 使用 sealed ephemeral handle，不进入普通变量或 durable 正文。
- 不增加 direct Provider bypass、per-delta durable journal、客户端 ACK/cursor、第二 terminal/conversation/callback ledger、数据库轮询或 gateway tool executor。
- OpenAI Chat SSE、Anthropic SSE、Responses SSE 与 Responses WebSocket 的解码正文必须与同一 Canonical / durable Answer Presentation 原样相等；正文不得作为事件身份。
- GitHub Actions 只运行仓库内单一 hermetic quality-gate command；本机现有 Claude Code、Codex、OpenCode 与真实本地应用只在冻结 assembly 后做 local acceptance，不联网更新或重编译客户端。
- Root agent 是唯一调度、assembly 与 Control Ledger owner；批准后先 fresh Scout 和 Work Packet freeze，全部开发与 fixture 装配完成后才允许一个 fresh Root QA。

无固定时间预算；仅记录 Packet、needs-split、assembly conflict、agent context、validation run 与 QA cycle 等可验证事件计数。
