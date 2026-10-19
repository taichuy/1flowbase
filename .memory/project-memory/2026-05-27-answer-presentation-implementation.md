---
memory_type: project
topic: answer-presentation-implementation
summary: Answer Presentation 用户可见输出层已在 runtime/compiler/API projection/前端文档校验中落地；2026-05-28 已完成 issue #501，waiting_prefix 输出快照从真实 node_runs 轨迹拆出为 answer_snapshot，UI 名称为 answer快照。
keywords:
  - answer-presentation
  - answer-template
  - openai-chat-sse
  - openai-responses-sse
  - native-sse
  - runtime-events
  - validate-document
  - answer_snapshot
  - issue-501
created_at: 2026-05-27 20
updated_at: 2026-05-28 08
last_verified_at: 2026-05-28 08
decision_policy: verify_before_decision
scope:
  - api/crates/orchestration-runtime
  - api/crates/control-plane
  - api/apps/api-server/src/routes/application_public_api
  - web/app/src/features/agent-flow/lib/validate-document.ts
---

# Answer Presentation Implementation

## 时间

`2026-05-27 20`

## 谁在做什么

AI 在当前 `main` 工作分支落地 issue `#497` 的核心实现，用户后续做最终人工验证。

## 为什么这样做

此前 OpenAI Chat / SSE 直接消费 LLM provider `text_delta`，导致 Answer 模板静态文本如分割线 `----` 不进入实时输出，terminal fallback 又会因为已有 text delta 被抑制。现在 Answer 节点作为用户可见输出真值，协议层只投影 Answer Presentation。

## 已落地边界

- 编译层从 Answer `answer_template` 推导 `AnswerPresentationPlan`，识别 `static_text` 与 `node_output` segment。
- 后端编译阻断同一输出重复引用，以及真实 happens-before 关系的反向展示；无依赖并行节点按 Answer 模板顺序允许。
- Runtime 用 `AnswerPresentationCursor` 只在当前 segment 匹配时投影 provider delta，后序并行 segment 缓冲到前序闭合后输出。
- 静态模板文本进入 durable presentation event；协议提示词、tool callback payload、provider metadata、`sys/env` 不进入 Answer Presentation。
- OpenAI Chat、OpenAI Responses、Native SSE 只把 `presentation.kind == "answer"` 的 `text_delta/reasoning_delta` 投影为用户可见内容。
- 前端 `validateDocument` 已补 Answer 模板重复引用与反向依赖顺序校验；并行引用不阻断。
- `complete_callback_task` 等离线 resume 路径会把 compiled Answer Presentation plan 带入持久化层；在 `WaitingCallback/WaitingHuman` 前只投影 checkpoint 中已完成节点可达的 Answer 前缀，避免 LLM2 工具等待时把 LLM1 与静态分割线吞掉。
- OpenAI Chat SSE 在 live stream 已转发 Answer delta 后，会推进匹配的 durable cursor，防止 durable drain 再补同一条 Answer 文本造成重复 chunk。
- `2026-05-28 00` 补齐等待态 Answer 节点物化：live continuation 与离线 callback resume / human resume 进入 `WaitingCallback` 或 `WaitingHuman` 前，会创建一个 `node-answer` 运行记录，输出当前 Answer 模板中已可达的前缀；当前缀非空时同步写入 `flow_run.output_payload.answer`。等待中的 LLM 自身输出仍不作为 completed prefix，避免把未完成节点内容提前混入 Answer。
- `2026-05-28 07` 用户确认后续清理方向：waiting_prefix 输出快照本身必要，但不应作为真实 `node_runs` 根级轨迹显示；新增 GitHub issue `#501` 作为 `#497` 的 L3 child，计划用接口字段 `answer_snapshot` / UI 文案 `answer快照` 承载，并挂到当前等待节点下方折叠展示。数据库历史记录先保留，不做 migration；不把快照塞进 LLM 工具调用 payload。
- `2026-05-28 08` issue `#501` 已实现：run detail response 会过滤 `materialized_from = "waiting_prefix"` 的虚拟 Answer node_run；仅当 flow run 当前处于 `WaitingCallback` / `WaitingHuman` 时返回 `answer_snapshot`，并关联等待节点。前端 API client / Agent Flow trace 映射消费 `answer_snapshot`，在等待 LLM 节点详情下显示折叠的 `answer快照`，不再在根 trace 显示虚拟“直接回复”。已完成的历史运行仍隐藏 waiting_prefix 虚拟记录，但不再展示过期 answer 快照；最终真实 Answer 节点继续作为真实 trace 显示。

## 验证证据

- `cargo test -p orchestration-runtime --quiet`
- `cargo test -p control-plane --quiet`
- `cargo test -p api-server sse --quiet`
- `cargo test -p api-server openai_chat --quiet`
- `cargo test -p api-server openai_responses --quiet`
- `cargo test -p api-server openai_chat --quiet` 覆盖 live Answer delta 不被 durable drain 重复输出。
- `cargo test -p api-server application_runtime_routes_stream_debug_run_returns_flow_accepted -- --nocapture`
- `cargo test -p api-server run_detail_response_`
- `pnpm --filter @1flowbase/web exec ../../scripts/node/exec-with-real-node.sh ../../scripts/node/cli/run-frontend-vitest.js run src/features/agent-flow/_tests/debug-console/run-detail-mapper.test.ts src/features/agent-flow/_tests/debug-console/debug-conversation-log-panel.test.tsx`
- `pnpm --filter @1flowbase/api-client build`
- `node scripts/node/cli/quality/test-contracts.js`
- `node scripts/node/tooling.js check-rust-backend`
- `pnpm --filter @1flowbase/web exec ../../scripts/node/exec-with-real-node.sh ../../scripts/node/cli/run-frontend-vitest.js run src/features/agent-flow/_tests/validate-document.test.ts`
- `pnpm --filter @1flowbase/web exec prettier --check src/features/agent-flow/lib/validate-document.ts src/features/agent-flow/_tests/validate-document.test.ts`
- `cargo test -p control-plane callback_tasks --quiet`
- `cargo test -p control-plane runtime_events --quiet`
- `cargo test -p control-plane continue_flow_debug_run_stops_at_human_input_and_persists_waiting_state --quiet`
- `cargo test -p control-plane --quiet`
- `cargo test -p api-server openai_chat --quiet`
- `cargo test -p api-server openai_responses --quiet`
- `cargo test -p api-server sse --quiet`
- `cargo fmt --check`
- 外部 OpenAI Chat SSE 冒烟：`019e6a29-b5be-7d82-80f7-00a34c89ff21` 首次等待生成空 `node-answer`；提交首个 Bash 工具结果后再次等待时，流式输出 LLM1 文本与静态分割线，并在 DB 中写入非空 `flow_run.output_payload.answer` 与第二条 `node-answer`。

## 注意

`http://192.168.31.25:7800` 外部 OpenAI Chat SSE 冒烟可连通，但当前运行服务返回重复 chunk；由于该服务未确认加载本工作树修改，不能作为本次实现通过证据。
