---
memory_type: feedback
feedback_category: repository
topic: 1flowbase 对外基座定位必须包含数据建模与扩展接口组成的应用后端
summary: 用户纠正：四大基座必须包含动态 Data Model、自动 CRUD API 与 Workflow Extension API 组成的应用后端；四者是围绕同一应用并列组合的能力，AI Gateway 是外部本地 Agent 可选接入的模型服务入口，不是应用内部或 MCP 操作的必经节点。
keywords:
  - 1flowbase
  - positioning
  - application backend
  - data model
  - CRUD API
  - workflow extension
match_when:
  - 修改 README、官网、GitHub Description、Topics、发布文案或产品基座图时
  - 总结 1flowbase 的核心基础能力或 Full-Stack Agent-Native 定位时
created_at: 2026-08-04 08
updated_at: 2026-08-04 08
last_verified_at: 2026-08-04 08
decision_policy: direct_reference
scope:
  - README.md
  - docs/READEME-i18n/README_CN.md
  - GitHub repository metadata
---

# 对外定位必须包含应用后端

## 规则

描述 1flowbase 核心基座时，不能只写 AI Gateway、MCP Gateway 和 Native React 前端区块。还必须覆盖由动态 Data Model、物理表与关系生成、自动 CRUD 运行时 API、Workflow Extension API 共同组成的应用后端能力。

“数据源”只表示外部连接与资源发现，不足以命名整个基座。最终公开名称需要产品确认；确认前可使用中性术语“应用后端 / Application Backend”。

四大基座不能画成强制串行流水线。外部本地 Agent 可以只连接 MCP Gateway，继续使用自己的模型配置来接管、搭建和管理 1flowbase；也可以选择把模型端点接入 AI Gateway，获得协议兼容、模型组合和详细日志。AI Gateway 不应被描述成应用内部调用模型、完成业务分类或使用 MCP 的前置依赖。

## 原因

遗漏这一层会让项目看起来仍是网关加 UI，无法解释应用的数据持久化、业务实体、标准 CRUD 与自定义后端接口从哪里产生，也削弱“开箱即用构建完整应用”的差异化。

## 适用场景

- README 与 GitHub 仓库元数据
- 官网 Hero、能力图和竞品对比
- 发布文章、视频脚本与项目介绍
- 判断项目是三基座、四基座还是 Full-Stack Agent-Native platform
- 编写四大基座架构图、完整应用案例或外部 Agent 接入路径
