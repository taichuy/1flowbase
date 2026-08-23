---
created_at: 2026-08-23 11
memory_type: project
decision_policy: verify_before_decision
scope: model provider plugin base services release
---

# Provider base services 发布与本地升级状态

## 谁在做什么

- 用户确认采用“官方插件声明能力并发布升级”的方向。
- AI 已在 `taichuy/1flowbase-official-plugins` 补发 Anthropic 0.1.38、DeepSeek 0.1.25、Gemini 0.1.21、OpenAI 0.2.29、OpenAI-Compatible 0.3.45；每个 Release 有六个平台包，官方 registry 已更新。

## 为什么这样做

- 已安装版本缺少 `config.validate` 和 `models.list` manifest capability，前端因此按契约隐藏“检测”。

## 当前阻塞

- 本地 API 的标准 `upgrade-latest` 下载 Release asset 连续三次返回 `extension_artifact_network_unavailable`（`error decoding response body`）；资产本身经直接 curl 验证可访问，当前未切换任何本地插件版本。

## 截止日期

- 未指定；在本地升级成功或用户决定允许签名包上传兜底前有效。

## 决策背后动机

- 保持 manifest capability 是唯一 UI 可见性真值，避免前端推断旧插件能力。
- 优先保持标准官方 registry 下载与 source kind；本地上传官方签名包会改变安装来源语义，须另行确认。
