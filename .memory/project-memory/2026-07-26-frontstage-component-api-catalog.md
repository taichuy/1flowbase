---
memory_type: project
topic: JSX Studio 可查询组件 API Contract 目录（#1459）
summary: 用户批准并启动 #1459 平衡方案；组件能力以插件安装时注册的结构化 TypeScript API contract 为后端真值，Ant Design 仅标注受控子集，JSX Studio 通过三列表格分页查询、插入注册片段并复制标准 API 文档，为未来 MCP 查询保留同一服务边界。
keywords:
  - frontstage
  - JSX Studio
  - component catalog
  - TypeScript contract
  - controlled Ant Design subset
  - MCP-ready
match_when:
  - 实施或验收 issue 1459
  - 修改 frontend component contract、antd-facade 组件类型或 JSX Studio 组件面板
  - 为外部 Agent 或 MCP 暴露组件能力查询
created_at: 2026-07-26 01
updated_at: 2026-07-26 01
last_verified_at: 2026-07-26 01
decision_policy: verify_before_decision
source_issue: "#1459"
scope:
  - api/plugins/capability-plugins/1flowbase/manifest.yaml
  - api/crates/plugin-framework
  - api/crates/domain
  - api/crates/control-plane
  - api/apps/api-server/src/routes/frontstage
  - web/packages/api-client
  - web/app/src/features/frontstage
---

# JSX Studio 可查询组件 API Contract 目录

## 谁在做什么、为什么

用户于 `2026-07-26 01` 批准 AI 实施 #1459。原因是现有组件卡片只从 `.d.ts` 猜名称，既不能告诉 AI 真实支持的参数规范，也无法作为未来 MCP 的可靠查询源。截止日期未指定。

## 已批准决策

1. 组件 description 描述实际支持的 props、类型、children/action/event 规则与限制，不写泛化用途口号。
2. 复用 Ant Design 的组件只声明为本地受控子集；上游信息用于溯源，本地注册 contract 拥有最终解释权。自定义组件明确标记 `custom`。
3. 使用标准 TypeScript interface 与 JSDoc/TSDoc，不发明 `@1flowbase-component`。复制内容包含真实 import、props、限制与示例。
4. contract 随插件模块安装注册并由后端分页列表、详情 API 查询；前端不再用正则推断或兼容 fallback。
5. JSX Studio 使用“组件 / Description / 操作”三列表格，操作为“插入 / 复制 API”。本期只完成 MCP-ready 查询边界，不实现正式 MCP。

## 当前状态

- 主体实现与定向自动化测试已完成；manifest、标准 declaration、目录筛选分页、API client、JSX Studio 交互和 TypeScript 检查均通过。
- `cargo check -p api-server --tests` 与 Rust 静态门禁通过；i18n hygiene 为 0 error，新增键无 warning。
- API route 运行集成测试因链接阶段两次超时尚无成功证据；当前会话没有 Playwright/page-debug 工具，浏览器可视行为等待用户验收。
- 未实现正式 MCP，未宣称完整 Ant Design 兼容，未 commit/push。
