---
memory_type: project
topic: /route 初始应用模板的数据范围
summary: 用户确认 /route 顶部栏目及全部子页面的初始模板只导入业务数据模型定义，不导入现有业务记录；应用日志也排除。
keywords:
  - route
  - portable template
  - initial template
  - data model records
match_when:
  - 修改 /route 初始模板或应用模板导出导入范围
  - 判断数据模型定义与业务记录是否随模板迁移
created_at: "2026-09-27 18"
updated_at: "2026-09-27 18"
last_verified_at: "2026-09-27 18"
decision_policy: verify_before_decision
scope:
  - deploy/docker/templates/route.1flowbase-template.json
  - api/crates/control-plane/src/portable_template
---

# /route 初始应用模板范围

## 时间

`2026-09-27 18`

## 谁在做什么

用户确认初始模板范围；Codex 正验证 `/route` 顶部栏目、六个子页面及关联应用、数据模型定义和 `1flowbase` MCP 实例的导出导入。

## 为什么这样做

用户计划把该模板作为应用初始模板，需要可在新 Docker 环境还原页面和接口定义。

## 为什么要做

源环境 `ai_gateway`、`node_template`、`node_config`、`daily_reports` 有 15 条现有记录；用户明确选择只迁移模型定义，不迁移记录；应用日志同样不进入模板。

## 截止日期

本轮验证完成时，无单独日期要求。

## 决策背后动机

初始模板应提供可复用结构，而不携带源环境运行数据。

## 关联文档

`deploy/docker/templates/README.md`；此目录是本地忽略目录，引用前需核对现有文件。
