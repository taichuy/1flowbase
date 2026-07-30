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
updated_at: 2026-07-30 18
last_verified_at: 2026-07-30 18
decision_policy: verify_before_decision
status: architecture_direction_aligned_live_ab_blocked
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
