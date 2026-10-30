---
memory_type: feedback
feedback_category: interaction
topic: codex-mcp-global-config
summary: Codex MCP 安装命令必须维持一次执行后将凭证全局持久写入配置文件的既定目标，不用临时环境变量方案替代。
created_at: 2026-10-30 23
updated_at: 2026-10-30 23
decision_policy: direct_reference
scope:
  - 1flowbase MCP 客户端配置
---

## 规则

诊断或修复 Codex MCP 复制安装命令时，保持将服务地址与 PAT 直接持久写入全局配置文件的目标；不要默认改成需要用户额外维护环境变量的接入方式。

## 原因

用户明确纠正了环境变量推荐，说明最初设计就是一次执行后全局配置；应修复失效机制，保留原有使用目标。

## 适用场景

Codex MCP 安装命令生成、认证失败诊断及后续修复。
