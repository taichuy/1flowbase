---
title: MCP Agent CLI Tabs 已批准
memory_type: project
status: active
decision_policy: verify_before_decision
created_at: 2026-07-11 00
updated_at: 2026-07-11 00
source: https://github.com/taichuy/1flowbase/issues/1244
---

# MCP Agent CLI Tabs 已批准

用户已批准在 MCP 客户端配置弹窗增加 `通用 / Codex / Claude Code / OpenCode` Tabs，并由后续实现者按 GitHub Issue #1244 落地。

这样做是为了让用户基于当前 MCP Endpoint 与 API Key，快速复制各 Agent 的用户级 MCP CLI 配置命令，同时保留原有通用 JSON 配置。

产品边界是 Agent Tab 只显示 CLI 命令，不提供配置文件片段；命令通过项目现有 Markdown 组件展示和复制，并在平台语法确有差异时区分 Windows CMD、PowerShell 与 macOS/Linux Shell。

实现前必须核对各客户端当时的官方 CLI 文档；若客户端 CLI 无法表达 HTTP Endpoint 与 Authorization Header，应停止并回到需求确认。当前没有承诺截止日期。
