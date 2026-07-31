---
memory_type: project
topic: Anthropic Claude Code-only 供应商的 AI Gateway wire fidelity 诊断
summary: AnyRouter 直连与 Gateway 差分已确认代理正常、new-api 会丢失拆分后的 beta、Gateway 会折叠当前用户 content block 并放大失败请求；架构方向已纠正为由 SourceProtocolContext 支持 Anthropic 同协议还原，不增设 Claude Code 专用协议或 profile。
keywords:
  - Anthropic
  - Claude Code
  - AnyRouter
  - new-api
  - wire fidelity
  - anthropic-beta
  - retry amplification
created_at: 2026-07-30 18
updated_at: 2026-07-31 09
last_verified_at: 2026-07-31 09
decision_policy: verify_before_decision
status: callback_resume_retry_fixed_mock_green_live_upstream_concurrency_limited
scope:
  - /home/taichuy/git/1flowbase
  - /home/taichuy/git/1flowbase-official-plugins
  - /home/taichuy/git/new-api
---

# Anthropic Claude Code Wire Fidelity Diagnosis

## 谁在做什么

Root agent 使用本机 Claude Code 2.1.220、tmux、同一“最近三次提交”案例，对 cc-switch 直连与 1flowbase Anthropic Gateway 做真实客户端、离线假上游和本地 new-api 源码差分。Provider beta 合并补丁只保留在隔离 worktree `anthropic-beta-coalesce-diagnostic`，尚未合并或安装。

## 为什么这样做

供应商只允许 Claude Code，且同机曾出现直连成功、Gateway 同时段失败。代理、协议 Header、正文归一化、客户端 session、HTTP/TLS 与重试可能同时变化，不能凭 429 单点归因。

## 已确认事实

- 原生 Claude Code 在 `2026-07-30 17:47` 与 `18:01` 均明确经 `127.0.0.1:7897` 的 Clash 节点连接 AnyRouter，仍返回 `429 / Service Unavailable`；当前 live A/B 不具判定力。
- 历史重叠窗口中直连成功而 Gateway 失败，粗粒度出口路由相同，因此代理缺失不是 Gateway 专属失败的充分解释。
- Gateway 将 `anthropic-beta` 发送为两条：第一条仅 `context-1m-2025-08-07`，第二条包含 `claude-code-20250219` 等残余 token。
- 本地 `new-api@66ee6b8f9889` 的 Claude adaptor 和 request-header clone 都使用 Go `Header.Get`，只取得第一条值；因此 Claude Code beta token 会确定性丢失。Provider 应至少合并成一个 Header。
- 离线完整正文捕获显示：Direct 当前 user message 保留两个 text content block，第二块带 `cache_control: ephemeral`；Gateway 将两块拼成一个字符串并丢失该 cache-control。Gateway 另外按应用配置增加 `，语言偏好中文` system block；这是工作流主动变换，不是协议 adapter 可透明恢复的残余字段。
- 同一个离线失败案例：Direct 只请求一次；Gateway 产生 2 个 FlowRun，每个 FlowRun 因 Opus 节点 `retry_enabled=true,max_retries=1` 再请求两次，共 4 个 Provider attempt。失败会被放大，不是首个拒绝的原因。
- UA、Stainless headers、Anthropic version、dangerous-browser-access、x-app、model、tools、reasoning 和 context management 的语义值在首轮离线对比中一致。

## 为什么要做

目标不是让 mock 通过，而是定义并验证一个可观察 contract：当源协议和目标协议都是 Anthropic Messages 且 AgentFlow 没有语义修改时，除 Provider origin、实际凭据值和必要的传输层重建外，出站请求应与入站请求协议等价。Claude Code 是 Anthropic 协议的真实验收客户端，不是新协议。

## 最新架构纠正

- `ProtocolContextEnvelope` 当前只保留 residual query/header/body，既排除 typed root fields，也过滤认证 header，无法完成同协议 round-trip reconstruction。
- 协议上下文应扩展为安全的 `SourceProtocolContext`：保留可还原的协议形状，认证只保留 presentation/scheme，不保留源 secret。
- Anthropic 同协议 Provider 应用 Provider 配置中的 secret 替换凭据值，并按源请求还原 `Authorization: Bearer` 或 `x-api-key` 的呈现形态。当前 Provider 无条件写入 `x-api-key`，这是默认策略，不是同协议不变量。
- AgentFlow 若主动改写 system/messages/tools 等语义，则出站请求只应产生与该改写对应的 semantic delta，并用 translation receipt 明确记录。
- 跨协议 Provider 仍从 AI Native 语义真值渲染目标协议，不盲目透传源协议形状。

## 截止日期与停止条件

无时间截止。等待原生 Claude Code 直连完整成功后，只执行一次 `transport-only` 和一次 `coalesced-beta` 串行 A/B。直连仍失败时停止真实请求；未经 live A/B 不合并或安装诊断 Provider，不宣称唯一根因闭合。

## 决策动机

优先修复确定性、低风险的 beta 单 Header 兼容；同时以完整 `SourceProtocolContext` 和同协议 round-trip invariant 作为根治边界。不再建立 `Claude Code compatible` profile；session/auth/body 的形状还原属于 Anthropic 协议上下文与同协议 Provider，TLS/代理/连接属于 Transport。

## 2026-07-30 22 新一轮真实运行证据

- 直连会话 `1588f433-e552-42a8-8170-da161ab61692` 在 `21:28` 后成功；Gateway 会话 `0f70e103-782b-4fae-8951-5f98c287b0e3` 在 `21:38` 后失败。两侧 prompt 和 Claude Code 版本相同，Provider Base URL 与 secret 已核对一致。
- `api-server` 与 `plugin-runner` 均配置 `HTTP_PROXY` / `HTTPS_PROXY=http://127.0.0.1:7897`，因此不是 Gateway 未走本地代理。
- Clash 日志确认直连成功使用香港家宽出口，Gateway 五次失败使用新加坡 Vless 出口；该 A/B 没有冻结出口线路，不能据此证明协议还原失败。
- Gateway 的 LLM 实际 system 有 5 个 block，前 4 个来自 Claude Code，第 5 个是工作流新增的 `，语言偏好中文`；因此也不满足“无 workflow semantic delta”的同协议 round-trip 前提。
- Gateway 产生 5 个串行 FlowRun，每个只有一个 `is_retry=false` Provider attempt，均在约 19～21 秒后收到 `429 / Service Unavailable`。当前重试修复已生效；五次请求来自 Claude Code 客户端层重发，不是 Gateway 内部放大。
- 确定的剩余缺口是失败路径可观测性：Anthropic Provider 只在成功读完 stream 后附加 request translation receipt，而 terminal cleanup 会清理原始 protocol context。非 2xx 运行因此无法事后证明最终 outbound logical wire 是否等价。

下一次 live A/B 必须同时固定同一 Clash 出口、同一 secret、同一精确 prompt，并清空 workflow system delta。只有在 logical wire 已证等价且仍仅 Gateway 失败时，才升级到 HTTP/TLS client fingerprint 或原生 transport passthrough。

## 2026-07-30 23 固定香港出口后的续聊证据

- 固定 `香港02丨直连` 后，Gateway FlowRun `019fb38c-ea9a-7d42-84d4-729615d556ec` 成功；同一 Claude Code 会话在一个 FlowRun 内连续完成 3 次 Provider 请求并执行 Git 工具链，模型均为 `claude-opus-4-8`。
- 续聊从 `15:06:28Z` 到 `15:12:14Z` 形成 18 个失败 FlowRun、19 次 Provider attempt；所有 attempt 的 request hash 都是 `sha256:7cee4a1a5c94622c8cb0b0214bf76b8435798c35038cccb3dbbab8b7df9858fe`，Clash 全程仍使用同一香港出口。
- 19 次结果为 1 次 `HTTP 502 Bad Gateway` 和 18 次 `429 / Service Unavailable`，全部在 first token 前失败。目标 FlowRun `019fb390-dd0c-7c03-9164-f1a6d0e3ed8f` 先收到 502，Gateway 按 5xx 策略重试一次；第二次收到 429 后停止。不是 Gateway 重试 429。
- 本地 `new-api@66ee6b8f9889` 的 Claude relay 对上游非 200 使用 `RelayErrorHandler`，保留 status/message 后投影为 Claude error；公开源码不生成固定的 `429 + Service Unavailable` 组合。该组合来自其后渠道或部署方 status-code mapping。
- 证据高概率指向上游渠道容量、限流或可用性波动，而不是 Gateway 随机改写同一续聊请求。由于失败路径仍缺少最终 outbound wire receipt，在未做同一续聊请求的 direct/Gateway 紧邻 A/B 前，不能把 Gateway continuation wire 风险降为零。

## 2026-07-30 23 续聊结论纠正

用户提供“新会话再次成功、同一会话续写再次稳定失败”的反例后，上一节的“高概率仅为上游波动”结论被更强证据取代：上游渠道亲和仍可能放大错误，但 Gateway 存在两个确定的响应协议 fidelity 缺陷。

- 两个独立 Claude 会话均重复同一模式：首次用户轮中的 2～3 次工具循环 Provider 调用成功，下一用户轮开始后稳定失败。
- Anthropic SSE projector 使用 `msg_{flow_run_id}` 作为 `message_start.message.id`。同一个 FlowRun 的 tool callback resume 会创建新的外部 `/v1/messages` 响应，但仍复用相同 message id。Claude 日志确认 tool-use 响应和 final-answer 响应拥有相同 `message.id`。
- 下一轮 Gateway 入站历史已经变为 `assistant(tool_use + final) -> tool(tool_result)`；translator 源码逐消息保持顺序，因此合并发生在 Claude Code 接收相同 message id 后，而不是 translator 主动合并。
- Anthropic Provider 只把 `thinking_delta` 转成 `ReasoningDelta`，忽略上游 `signature_delta`；Provider result 虽在 metadata 中保存 `message_id`，但 `response_id` 固定为 `None`。Anthropic SSE projector又主动生成 `signature: ""`，且不发送 signature delta。
- 实际续聊历史中的两个 reasoning block 均携带空 signature。Provider 会把该空值重新渲染为 Anthropic thinking block；这是只在第二轮出现的确定 wire 差异。
- 本地 new-api 默认 Claude affinity 使用 `metadata.user_id`，两个新会话的 end-user reference 不同，因此不同会话可能命中不同渠道；这仍是残余上游变量。但无论供应商是否掩盖错误为 `429 / Service Unavailable`，Gateway 都必须先修复 message identity 与 opaque thinking signature 的响应往返。

后续 Root 应从“请求侧 SourceProtocolContext round-trip”扩展为“Anthropic 多轮请求 + 响应 round-trip”：每个外部响应使用独立/真实 message id；thinking signature 从上游 Provider 经 contract/runtime 到 Anthropic SSE 原样、临时传递，不持久化或日志化；以真实 Claude Code `最近三次提交` + 续问作为 live AC。

## 2026-07-31 多轮响应 round-trip 修复与验收

- 主仓已合并至 `dev@1e126cb08bd3d4f2a0b4c75e3a30c3b3d8975ef0`；official Provider 已合并至 `main@66ca46a6953c54bd0237395dbc94e6626ffa1561`，Anthropic manifest 版本为 `0.1.32`。
- 每个外部 Anthropic response 现在生成独立 `msg_<uuid-v7>`；同一 response 内 mapper 持有该 ID，tool callback resume 不再复用 FlowRun ID。
- official Provider 解析 `signature_delta` 为专用 `reasoning_signature_delta` wire event。Host 通过 required live lane 有序传递，并投影回 Anthropic SSE；该事件为 ephemeral、`persist_required=false`、`trace_visible=false`，显式排除 durable events、canonical answer、coalesced observability 与日志。
- dev mock 的 Claude-only 两用户轮 vector 会发送 opaque thinking signature，后续 callback/续聊必须原样带回；artifact 只记录 `thinkingSignatureMatched` 布尔值。跨外部响应 message ID 必须唯一。
- 集中证据：主链 `reasoning_signature` 4 条 Rust 定向测试通过；message identity 1 条通过；Provider signature 1 条通过；Node conformance 72/72 通过。证据目录在 assembly worktree 的 `tmp/test-governance/anthropic-multiturn-qa4/` 与 `anthropic-multiturn-qa5/`。
- 本地正式 package/install 已安装 Anthropic `0.1.32`。克隆实例时 `proxy_url` 也是 secret，必须与 `api_key` 一样通过 secret reveal 重建；把列表中的掩码写回会确定性得到 `invalid proxy_url`。
- 现有 `any` 工作流还包含不可运行的 `z-ai/glm-5.2` Anthropic 节点；编译器会对整张图 fail closed，并把 `provider_instance_not_ready` 粗化为 `invalid input: provider_code`。这属于现有工作流配置/错误归一化问题，不是本次响应协议修复。
- 专用 start→LLM conformance 应用已证明请求进入配对 Provider。无 `[1m]` profile 时上游返回明确 400，要求启用 1m；改用 `claude-opus-4-8[1m]` 后，上游返回 `429 Service Unavailable`。按停止条件未继续重试，因此真实两轮 AC 仍未完成，不能宣称 live green。
- 本地验收后已恢复原 Anthropic `0.1.31` assignment、移除现有 `any` draft 的临时 source pin，并删除三个位于证据中的临时 conformance 应用；避免 0.1.32 assignment 在缺少 instance migration 时破坏其他本地 Anthropic instance。0.1.32 package 与克隆实例保留为未分配验收资产。

## 2026-07-31 09 Claude Code 重试被 callback resume 幂等吸收

- Gateway 会话 `4ed93813-a46c-4cee-9578-59fba13ca2b1` 的同一首轮请求先形成两个首 token 前 429 FlowRun，Claude Code 均实际重发；第三个 FlowRun `019fb5b1-8f16-7372-8ab1-a556ed6bb3c8` 首次 Provider 调用成功并返回 Bash tool use。
- tool result resume 的第二次 Provider 调用在首 token 前收到 429。该 FlowRun 随即失败，但 callback resume attempt 被记录为 `succeeded`，含义只是 callback payload 已消费，不代表模型推理成功。
- Anthropic SSE 已先以 HTTP 200 打开，失败只能投影为 SSE `error`。Claude Code 2.1.220 因此进入 non-streaming fallback，并按默认 10 次重试；观察到的约 186 秒与 0.5/1/2/4/8/16/32 秒封顶指数退避吻合。
- 后续相同 tool-result 请求命中 `callback_task_id` 唯一幂等记录，`resume_existing_attempt` 每次返回同一个 terminal-failed FlowRun；期间没有新 FlowRun、callback attempt 或 Provider attempt。因此客户端在重试，但 Gateway 将全部重试吸收为缓存失败，上游没有再次被访问。
- 直连没有 Gateway callback 状态，429 后每次 HTTP retry 都重新到达上游，故不稳定窗口内仍可能恢复成功。根因不是代理或请求 wire，而是 Gateway 把“callback 已消费”和“推理已成功”合并为一个幂等结果。

## 2026-07-31 10 Callback retry 修复与验收

- callback admission 已改为 outcome-aware：已有 callback attempt 仍负责证明 payload 只消费一次；若对应 FlowRun 明确为首 token 前失败，且错误为 429 / 5xx、`rate_limited` 或 `endpoint_unreachable`，control-plane 返回 `StartNewTurnFromHistory`，route 用完整历史创建新的 inference turn。after-token、401、changed payload 与缺少 first-token fact 均 fail closed。
- orchestration runtime 将 `failed_after_first_token` 写入 durable Provider error payload；Anthropic SSE 将 429 / `rate_limited` 映射为 `rate_limit_error`，529 映射为 `overloaded_error`。
- 集中自动化证据：Rust `anthropic_callback_retry` 4/4、Node 定向 61/61、`cargo fmt --check`、Node syntax 与 `git diff --check` 均通过。
- Claude-only 隔离 mock vector `tools-callback-retry-after-429` 真实使用本地 Claude Code 2.1.220：Provider outcome 为 `completed -> http-429 -> completed`，durable run 为一个 failed 与一个 succeeded，客户端 Read 只执行一次；证明失败 resume 后的客户端 retry 会重新到达 Provider，而不会再次消费 tool result。
- 固定当前 Gateway 与香港代理的真实会话 `1f4b39e7-f81a-4ca6-8fb8-b9464a60c260`：第一问“查看最近三次代码提交”成功，并在同一 FlowRun 完成 Bash callback；第二问的父 FlowRun 两次 Provider 调用均成功，随后 Claude Code 自行启动 `Agent/Explore` 子会话。该子会话在供应商并发限制下连续 8 个独立 FlowRun 返回首 token 前 429；每次 retry 都产生新 FlowRun 并重新到达 Provider，证明旧的 retry absorption 已消失。按上游停止条件中止，未取得第二问最终文本。
- 当前结论：本轮确定的 Gateway callback retry 缺陷已修复；真实第二问未完成的直接原因是 Claude Code 自行并发的 Explore 子会话遭上游容量/并发限制，不是映射协议、代理缺失或 callback 幂等吸收。中止后的父 FlowRun 保留为 `waiting_callback`，公开 cancel endpoint 返回 409，未直接改库清理。
