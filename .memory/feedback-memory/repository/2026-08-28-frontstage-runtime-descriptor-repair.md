---
memory_type: feedback
feedback_category: repository
topic: Frontstage Block 运行时描述缺失
summary: 对预览失败的已创建 Block，优先以同页可用 Block 的运行时身份为模板做最小批处理修复，不抢先改动后端重构。
keywords:
  - frontstage
  - block
  - runtime_descriptor
  - preview
match_when:
  - MCP 创建的 Frontstage Block 因 catalog 或 contribution 为空而无法预览
created_at: 2026-08-28 10
updated_at: 2026-08-28 10
last_verified_at: 2026-08-28 10
decision_policy: direct_reference
scope:
  - Frontstage Block Tree MCP
---

# Frontstage runtime descriptor 临时修复

## 时间

`2026-08-28 10`

## 规则

对同类预览问题，先比较可用与失败 Block 的 `runtime_descriptor`；使用脚本只补齐缺失的运行时身份字段，并逐块回读验证。

## 原因

用户确认此处理方式适合后端仍在重构期间的临时修复，可避免不必要地介入后端实现。

## 适用场景

Frontstage 页面中的已创建 Block 因 catalog、contribution 或兼容运行字段缺失导致 native React 预览失败。
