---
memory_type: feedback
feedback_category: interaction
topic: 通过 Workflow 扩展已有数据接口
summary: 数据存在但缺少专用接口时，先验证 Workflow 应用的 SQL、代码节点与 extension 发布能力，不直接判定必须修改产品源码。
keywords: [workflow, MCP, 低代码, 聚合接口]
match_when: [通过 MCP 构建低代码页面, 已有数据缺少专用查询或聚合接口]
created_at: 2026-09-06 08
updated_at: 2026-09-06 08
decision_policy: direct_reference
scope: [Workflow, Frontstage, MCP]
---

## 规则

已有数据缺少专用接口时，优先通过 Workflow 应用组合查询、SQL 和代码节点，并发布 extension 接口；逐项验证发布、MCP 调用及低代码调用，不把缺少现成 Rust route 等同于必须修改源码。

## 原因

用户明确指出 Workflow 本身就是扩展接口的能力入口，应先使用平台能力完成配置。后端提供数据真值不意味着每个新查询都需要新增产品代码。

## 适用场景

通过 MCP 创建 GUI、报表和业务接口。只有经当前运行证据确认 Workflow 或页面调用链存在配置无法解决的缺口时，才按用户边界暂停并说明源码改动需求。
