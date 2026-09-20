---
title: "Issue 2085 三层 Gateway 故障修复与审计分工"
memory_type: project
created_at: "2026-09-19 17"
updated_at: "2026-09-20 10"
decision_policy: verify_before_decision
status: active
tags: [gateway, responses, audit, issue-2085]
---

- 用户已确认平衡方案，要求创建 Root 与 Delivery，并将实现交给新的开发会话；当前会话专门审计，不直接接管实现。
- 唯一活动计划：https://github.com/taichuy/1flowbase/issues/2085 ；Delivery 为 #2086、#2087、#2088，已经建立 GitHub sub-issue 关系。
- 目的：保持映射协议层 → AI Native → 供应商层职责，修复本次原始错误被回执/终止分类覆盖的问题，复用既有恢复机制，完成插件版本升级、本地编译替换、Codex 真实验证与云端发布证据。
- 动机：避免继续通过扩大重试或超时掩盖故障，也避免重复建设 #2072/#2075/#2079 已有机制。
- 开发会话维护 Root Control Ledger、隔离 assembly 和唯一集中 QA；当前会话独立审计，用户最终验收 Root。
- 截止日期：用户未设定；有效期至本 Root 用户验收或明确替代。阶段、SHA、验收结果以当前 GitHub Root 为准，不以本记忆替代执行账本。

## 2026-09-20 有界恢复实现轮（已批准方案 = 现有状态机内的有界恢复，不重新讨论方向）

- 谁在做什么：开发会话按 `tmp/test-governance/issue-2085/audit-websocket-hour/report.md` 的缺口实现并自检，
  交接在 `tmp/test-governance/issue-2085/fix-bounded-recovery/`；原审计会话独立审计，用户人工长测后验收。
- 关键根因（修复前）：插件在**存在 directive** 时把 `cursor_provenance=None` 判为 `OpaqueUnowned`，
  `ProxyFailed`(1011) 因此直接 `TerminalInterruption`；宿主 `llm_executor` 的 `invoke -> Err` 分支固定传 `None`，
  插件已放进 `provider_details` 的 typed recovery receipt 永远不会被 AI Native 消费。
- 候选：宿主 `dev` @ `c2c941a46b47bd84bd023ec1d62282f2e641ba38`（**本地提交，未推送、未激活**）、
  插件 `main` @ `897c13f2f85d71a9ecdd6761f8423bb291d65e40`（rebase 前 `7ef1d55`，`openai@0.2.40`，**已推送并完成云端发布**）。
  宿主候选 binary `53062dec…`；插件本地 gnu 包 `368f2cb7…`（本机 musl std 与工具链不匹配不可用）。
- 云端发布（2026-09-20，用户明确授权推送后自动触发）：release `openai-v0.2.40`，`provider-release` run
  `35482180834` 9/9 job success，6 平台包摘要与 `official-registry.json` 逐项一致（`latest_version=0.2.40`，
  索引 commit `faae1a4`）；`provider-ci` run `35482180856` success。只发布插件，**不宣称长会话稳定性通过**。
- 范围提醒：插件 0.2.40 单独安装即可修复事故主路径（managed directive 无 provenance + 1011 → 插件内同 epoch
  重连并在新 socket 重发游标请求）；宿主候选未推送时，失败侧 AI Native 审计保真度仍是旧行为。
- 语义决定：directive 的 `cursor_provenance=None` 只表示「host 无绑定声明」，游标与物理连接 owner 归供应商；
  host 一旦声明 `ConnectionBound` 仍走严格校验。终态 receipt 允许不带 socket incarnation（非终态仍必须带）。
  宿主把「存在但不可信的回执」记为 `InvalidReceipt`，不再塌缩成 `MissingTypedReceipt`。
- 验证：最小验收项 1–6 自动化通过（含受控变异证明 managed 回归测试可拒绝修复前实现）；既有失败已用基线复现，
  非本轮回归（orchestration `answer_and_failover` 3 项、插件 1 项、runtime-extension-host 12 项缺 fixture 环境变量）。
- 未结算：真实 CLI 短程（项 7）未执行——7800 是用户运行中任务、7900 属另一会话、缺本任务隔离应用配置；
  >65 分钟人工长测（项 8）待人工；首次 1011 底层触发原因仍未证明，方案不依赖「一小时必断」假设。
- 结论口径（用户授权推送后更新）：**开发与已列自动化验证完成，插件已云端发布；长会话稳定性待人工验收。**
  云端打包成功不等于整体通过，不得据此宣称长会话稳定性已通过。
