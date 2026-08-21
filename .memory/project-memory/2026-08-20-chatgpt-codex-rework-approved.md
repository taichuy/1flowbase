---
memory_type: project
topic: ChatGPT Codex Provider 重构 Issue Tree 已批准
summary: 用户于 2026-08-20 17 批准将插件技术身份统一为 chatgpt-codex，采用创建/编辑共用的分步 PKCE 授权，新增通用 Provider usage/rate-limit window contract，以及查询、次数、重置三动作；新 Root #1797 已建立，旧 #1776/#1777/#1778 已 superseded 并关闭。
keywords:
  - chatgpt-codex
  - provider auth
  - rate-limit window
  - reset credit
  - issue 1797
match_when:
  - 实现或验收 ChatGPT Codex Provider 重构
  - 调整 Provider usage 或 reset-credit contract
  - 回看旧 ChatGPT Subscription Issue Tree 的替代关系
created_at: 2026-08-20 17
updated_at: 2026-08-20 23
last_verified_at: 2026-08-20 23
decision_policy: verify_before_decision
scope:
  - https://github.com/taichuy/1flowbase/issues/1797
  - https://github.com/taichuy/1flowbase/issues/1793
  - https://github.com/taichuy/1flowbase/issues/1794
  - ../1flowbase-official-plugins/runtime-extensions/@taichuy/chatgpt-codex
---

# ChatGPT Codex Provider 重构

## 谁在做什么

后续执行以 Root #1797 为唯一活动真值；Delivery #1793 负责 `chatgpt-codex` identity、分步 PKCE 授权与草稿实例生命周期，Delivery #1794 负责动态模型、5h/7d 已用百分比、reset credit 次数查询和显式重置。旧 #1776、#1777、#1778 已标记 superseded 并关闭，只保留历史实现和 QA 证据。

## 为什么这样做

旧交付的 `chatgpt` / `ChatGPT Subscription` 命名、仅在已有实例后出现的通用授权动作卡，以及缺失额度查询与 reset 操作，不符合用户希望的直接产品体验。重构需要同时闭合认证、动态检测和额度操作，而不是继续在旧 Issue 上追加补丁。

## 决策与动机

- 技术 ID、Provider code、包和 release identity 使用 `chatgpt-codex`，展示名为 `ChatGPT Codex`。
- 授权采用“生成链接 → 浏览器授权 → 粘贴 callback URL/code → 完成授权”，不扩展 RT、auth.json 或 Personal Access Token。
- 宿主增加通用 usage/rate-limit window 与 reset-credit contract；ChatGPT endpoint 和 payload 仍归插件。
- UI 只显示 5h、7d `used_percent`，并提供 `查询 / 次数 / 重置`；次数为 `available_count`，重置需二次确认、一次最多消费一个 credit，成功后刷新额度和次数。
- 真实历史实例若需要 secret migration，或 reset consume 无法证明单次消费安全，停止实现并回到 Root 讨论。

## 截止日期

无固定截止日期；Root 当前为 `phase:ready`，下一步按 long-running-work 完成只读 Scout、packetization、assembly 与唯一集中 QA。

## 2026-08-20 源码交付状态

- Root assembly 已完成并合并推送：主仓 `dev@5daf1c9b0d1dc8e26b29ab30903312b5aa50d7df`；官方插件 source `a44b48d05535ed0bcd4d8490afbb85a38009aeeb` 已发布，自动生成的官方 Registry 位于 `main@74afe91ced1806f7149cfbad10e6813e0673095a`。
- 三轮集中 QA 后，#1797 引入范围的 Rust provider contract / host / control-plane / storage、API route fixture、前端 drawer / consumer、API client、official ChatGPT Codex 插件、catalog / registry、i18n 与 Rust static evidence 均通过；完整日志位于 assembly 的 `tmp/test-governance/1797-qa3-*`。
- `api-server` 同批仍有一条旧 settings family deletion 测试断言 `409`，当前返回 `200`。它与 #1785 已批准的“保留 installation / instance，仅卸载本地 artifact”语义冲突，且 #1797 未触及其删除路径，作为 existing stale expectation warning，不阻断本 Root。
- #1797 Control Ledger 已记录 `SOURCE_QA_PASS + CI_PACKAGE_PASS`：`provider-release #32370884724` 成功，Release `chatgpt-codex-v0.1.0` 已上传 6 个跨平台、带 SHA-256 digest 的包。关闭 Root 前仍需用户在真实账户完成 PKCE OAuth、usage 和显式 reset UAT。

## 2026-08-20 安装包修复交付

- 用户批准以 Root #1797 下的 #1810 / #1811 完成安装包完整性与上传弹窗边界修复；主仓 `dev@766a38a9f27bc058e60d7b482a3b4c2ad531ccbe` 与官方插件 `main@c2be0bc4e6e5c8fd15485d3ffe452ee13ca57141` 已合并推送。
- 旧打包器曾短暂发布错误 identity 的 `v0.1.1` 资产，零下载且已在重发前删除；正确发布由 `provider-release #32377364498` 使用主仓 `dev` 作为 packager ref，六个平台与 registry job 均成功。
- 独立验包确认 Linux AMD64 `v0.1.1` 中 `manifest.runtime.entry=bin/chatgpt-codex-provider` 与实际归档路径一致；`_meta/official-release.json` 的 `plugin_id`、`provider_code`、版本均为 `chatgpt-codex` / `0.1.1`，归档 SHA-256 与官方 registry 一致。后续仅需用户在当前 `dev` 人工上传该官方包并运行安装后 UAT。
- UAT 发现上传 Modal 在未点击提交时显示失败 Alert；根因是空 mutation error 被泛化为失败文案，且 reset 未清除 mutation error。主仓 `dev@a9e53daee` 已修复并加入回归：空错误不展示，打开/关闭/重选文件均清除历史错误；真实 package UAT 仍待用户继续。
- 后续 UAT 发现 `Upload.Dragger` 的内置文件列表侵入拖拽框底部并与提交按钮视觉重叠。主仓 `dev@df5abd986` 已改为 Dragger 外的独立 selected-file 容器；结构回归和 `component.plugin-upload-install-modal` style-boundary 均通过。
- 用户确认上传成功后应立即关闭 Modal 并刷新整页；主仓 `dev@54df9ed82` 已在 mutation 成功且现有查询失效完成后通知页面执行该动作，失败路径不刷新。定向 mutation 回归与既有 Modal 回归均通过。
- 用户确认模型供应商的上传仅替换官方安装流程的下载步骤；上传后必须由后端继续“启用 → 分配到当前 workspace”，但不得自动创建 Provider 实例或写入 OAuth/API Key。主仓 `dev@583a23b54` 已合入并推送：上传路由调用专用 service，复用官方安装的后置生命周期，认证路由回归断言 `desired_state=active_requested`、`task_kind=assign`，官方安装回归与格式检查均通过。此前已上传且 disabled 的安装不做自动迁移，UAT 可重新上传或使用既有“安装到当前 workspace”补救。
