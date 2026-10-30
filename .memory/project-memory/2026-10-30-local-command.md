---
title: "本地一次性命令接口：可配置角色授权、无启用配置、三平台原生验证已完成"
memory_type: project
created_at: "2026-10-30 08"
updated_at: "2026-10-30 12"
decision_policy: verify_before_decision
status: active
tags: [local-command, issue-2715, user-acceptance]
---

- 用户已确认修订并授权实施，Windows 测试必须交由 GitHub Actions（本地没有 Windows）。本会话完成 Single Issue #2715：https://github.com/taichuy/1flowbase/issues/2715 ，保持开放等待用户验收。没有约定额外交付截止日期。
- 阶段目标是普通可授权的一次性后端本地命令 API：只接受 command 字符串，沿用其他权限选择操作；不额外 root-only，不要求环境变量启用，不按猜测设置命令期限或输出上限。这样使接口符合既有权限架构，避免无必要部署门槛。
- 已完成修订并合入、推送 dev；具体 SHA 与证据见主目录 tmp/test-governance/local-command-portable/acceptance.md 与 Issue。原生生产执行模块在 Windows/Linux/macOS Actions 全部通过；Linux 真实 API 权限与输出测试通过；独立只读检查无 blocker。
- 当前证据仅覆盖生产执行模块与 Linux API；完整 Windows/macOS 发行包、Windows 10 实机和全仓 CI 未验证。Windows 实测 runner 为 windows-2025，不能把它写成 Windows 10 实机通过。
- 未部署或重启服务，7600 未操作。旧 root-only、环境变量启用和 Unix-only 交付及其验收结论已被本修订取代，后续不要恢复这些限制。
