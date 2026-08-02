---
memory_type: project
topic: AI Native CountTokens Provider 专属计数与 total fallback Root
summary: Root #1556 已完成双仓本地集成、完整 AI Gateway 门禁和真实 Claude cycle30 验收，AC-001～010 全部 green；共享开发环境也已通过正式 switch-version 恢复 DeepSeek 0.1.18 一致状态，当前等待用户人工验收。
keywords:
  - count_tokens
  - ai-native
  - provider-plugin
  - generic-estimator
  - ai-gateway
  - issue-1556
match_when:
  - 继续规划或实现 CountTokens 本地计算、Provider capability 或错误投影
  - 修复 Claude Code CountTokens 导致的 502 或会话中断
  - 为官方 Provider 增加 Tokenizer、估算器或网关门禁
created_at: 2026-08-01 16
updated_at: 2026-08-02 07
decision_policy: verify_before_decision
status: active
source_of_truth: https://github.com/taichuy/1flowbase/issues/1556
---

# AI Native CountTokens Total Fallback Root

## 谁在做什么

Root agent 已完成主仓与官方 Provider 的隔离 assembly、本地集成与集中 QA。CountTokens 定向 fixtures、完整 AI Gateway blocking gate、六 Provider actual-package conformance 和真实 Claude cycle30 均通过。Root #1556 与 #1557～#1559 已进入 `phase:user-acceptance`。

## 为什么这样做

工作流对外是虚拟模型，CountTokens 是 Claude Code 等客户端的辅助预检。Provider 不支持、上游计数端点不可用、本地 Tokenizer 缺失、插件失败或未知多模态都不应升级为 502 并中断会话。

## 已确认方向

- Provider 插件按实际目标供应商和模型负责官方接口、模型 Tokenizer 与 Provider-family estimate。
- CountTokens 与 Generate 共用完整 Canonical Prompt Envelope，不能丢失 system、tools、MCP tools、response format 或媒体语义。
- Plugin Framework 提供不含供应商知识的 generic estimate；正常非空估算必须大于 0，估算器自身也失败时允许返回 0 哨兵。
- 内部 receipt 记录 method、coverage、unknown block 与 fallback reason；外部兼容协议保持标准 `input_tokens`。
- Usage 只用于事后偏差校准，不成为 Provider 插件在线依赖；工作流 publication 不冻结 Provider capability。

## 最终结果与根因

- CountTokens 定向 fixtures：47/47 PASS。
- 完整 AI Gateway blocking gate：PASS，覆盖 249 个 Node gate tests、Rust CountTokens 矩阵、Generate/stream/callback/runtime 和六 Provider actual packages。
- 真实 Claude cycle30：`input_tokens=155`，initial 与同 session follow-up 均 PASS；DeepSeek 0.1.16→0.1.18 无 republish，cleanup PASS。
- 最终根因不是 Gateway 或 Provider wire：旧验收 runner 使用 `enable + assign` 切换版本，只更新 `plugin_assignments`，没有迁移 `model_provider_instances.installation_id`。runner 已统一改用正式 `switch-version` 控制面。
- 共享开发库已通过正式控制面依次切到 0.1.16、再回 0.1.18；assignment 与 Provider instance 最终一致，publication 不变。

## 线上发布

- 主仓 CountTokens assembly 已推送到 `origin/dev`，后续 `dev` 提交仍包含该集成点。
- 官方插件源码已推送到 `origin/main`，线上 `provider-release` 与 `provider-ci` 均通过。
- Anthropic 0.1.35、OpenAI 0.2.24、DeepSeek 0.1.18、Gemini 0.1.19、Aliyun Bailian 0.1.13、OpenAI Compatible 0.3.41 均已发布。
- 每个版本包含 darwin/linux/windows × amd64/arm64 共 6 个签名 `.1flowbasepkg`；`official-registry.json` 与 runtime catalog 已由发布自动化回写。

## 截止与复核

- 截止日期：未指定。
- 当前复核 #1556 的 `phase:user-acceptance` 正文；用户人工验收完成后关闭 Root 与 Delivery。
