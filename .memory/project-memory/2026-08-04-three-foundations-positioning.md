---
memory_type: project
topic: 1flowbase Agent-Native 应用平台基座定位
summary: 首轮三基座定位遗漏应用后端；用户已确认升级为 AI Gateway、MCP Gateway、Application Backend 与 Native React frontend blocks 四大基座，并明确 AI Gateway 是外部本地 Agent 可选接入的模型服务入口，四者不是强制串行链路。
keywords:
  - 1flowbase
  - agent-native
  - AI Gateway
  - MCP Gateway
  - Native React
  - frontend blocks
  - GitHub positioning
match_when:
  - 修改 1flowbase README、官网定位、GitHub Description、Topics、发布文案或竞品叙事时
  - 判断 AI Gateway、MCP Gateway、Native React 前端区块属于已实现能力还是路线图时
created_at: 2026-08-04 08
updated_at: 2026-08-04 08
last_verified_at: 2026-08-04 08
decision_policy: verify_before_decision
status: active
scope:
  - /home/taichuy/git/1flowbase
  - https://github.com/taichuy/1flowbase
---

# 三大基座统一定位

## 谁在做什么

用户确认 1flowbase 已完成三大基础能力：AI Gateway 负责兼容协议转换、路由和分发；MCP Gateway 让 Agent 发现、管理和搭建 1flowbase 应用；Native React 前端区块承载人机交互界面。项目对外定位统一为开源 Agent-Native 应用平台。

## 为什么这样做

旧 README 与 GitHub Description 只突出多模型 AI Gateway，并明确写着 MCP Gateway 尚在路线图中，已经与当前源码和产品阶段冲突。统一三基座叙事可以让开发者从 `agent-native`、`AI gateway`、`MCP gateway`、`React` 等真实搜索入口理解项目。

## 为什么要做

近 30 天品牌精确检索几乎只有仓库自身，而相邻讨论正在集中到 Agent control plane、MCP Gateway、MCP Apps / interactive UI 与 agent-native applications。项目需要先让 GitHub 首屏准确表达已实现差异，才能承接这些问题空间流量。

## 截止日期

无固定截止日期；`2026-08-04` 已完成首轮 GitHub 定位切换。

## 决策背后动机

不用尚未验证的 MCP Apps 协议兼容或实时 Generative UI 蹭热点，而是用可由当前源码直接证明的三大基座建立差异化：Agent 通过 MCP 操作应用，人通过 Native React 界面使用应用，兼容客户端通过 AI Gateway 调用应用。

## 后续验证边界

项目记忆超过两天后，继续引用“已实现”结论前应回看当前 README、MCP protocol routes、MCP management routes、frontend block SDK 与 Native React runtime；若能力边界变化，以最新源码和运行证据为准。

## 2026-08-04 应用后端遗漏纠正

用户指出首轮“三大基座”遗漏了完整后端层：1flowbase 可直接定义 Data Model，物化 PostgreSQL 表、字段、索引和关系；已发布 Data Model 自动生成 list/create/get/update/delete 运行时 API；Workflow 可以发布 `/api/ex/{slug}` 自定义扩展接口。

当前事实已由 `physical_schema_repository.rs`、`runtime_data_model_docs.rs`、runtime model CRUD routes 与 `workflow_extension.rs` 取证。用户已确认公开名称使用 `Application Backend / 应用后端`，形成四大基座；不要只叫 `Data Source`，也不要在未满足托管服务预期前直接宣称完整 BaaS。

四大基座关系固定为并列可组合：MCP Gateway 是外部 Agent 控制面；Application Backend 是数据与 API 面；Native React frontend blocks 是人机体验面；AI Gateway 是外部客户端按需接入的模型服务面。外部本地 Agent 不接 AI Gateway 也能通过 MCP 接管 1flowbase；接入后增加兼容模型接口、模型组合与详细执行日志。
