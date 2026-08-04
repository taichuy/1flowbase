---
memory_type: project
topic: 1flowbase 三大基座完成并统一为 Agent-Native 应用平台定位
summary: 用户确认 AI Gateway、MCP Gateway 与 Native React 前端区块三大基座已经完成；GitHub README、Description 与 Topics 已改为 Agent-Native application platform 统一定位，后续对外表达不再把 MCP Gateway 写成路线图能力。
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
