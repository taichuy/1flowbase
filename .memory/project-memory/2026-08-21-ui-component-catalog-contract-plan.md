---
memory_type: project
topic: UI 组件目录与模块 export 边界修复计划
summary: 已按已批准 Single Issue #1823 合入 dev：模块 exports 只表示可 import 符号，组件目录只展示完整契约；后台使用结构化组件契约表单。AC-001～AC-008 已有集中 QA 证据，等待用户验收。
keywords:
  - ui-management
  - component-catalog
  - module-exports
  - insert-snippet
  - structured-form
created_at: 2026-08-21 22
updated_at: 2026-08-21 22
last_verified_at: 2026-08-21 22
decision_policy: verify_before_decision
source_issue: "#1823"
scope:
  - api/crates/control-plane/src/ui_management.rs
  - api/apps/api-server/src/routes/frontstage/component_capabilities.rs
  - api/apps/api-server/src/routes/settings/ui_management.rs
  - web/app/src/features/settings/components/ui-management
  - web/app/src/features/frontstage/components/jsx-studio
---

# UI 组件目录契约边界交付

## 谁在做什么

实现已在隔离 worktree 完成并合入当前 `dev`，提交为 `ae838fd9ea8d0016a7e41b89bb7dc369fcdd8561`；线上 #1823 已回写集中 QA 证据，现等待用户独立验收。

## 为什么这样做

提交 `53df6afe9` 将所有已注册 module export 暴露为组件候选，并在缺少契约时插入裸 `export_name`，把可 import 符号与可插入组件混为一谈。后台组件契约又默认使用原始 JSON 编辑，偏离结构化管理预期。

## 已批准边界

- 官方 manifest 是模板、模块和组件契约代码真值；`inherit` 随官方更新，用户 published override 独立持久化。
- Frontstage 组件目录只展示完整有效契约；无契约 export 仍可通过源码静态 import 使用。
- 后台使用结构化表单管理契约，并允许为已安装、当前节点可执行的 export 新建用户契约。
- 不包含在后台编写、编译或发布新的运行时组件实现，不做历史数据清理或 migration。
- 不关闭 Issue，保持 `phase:user-acceptance`，等待用户独立审核。

## 截止与验收

线上 issue #1823 的 AC-001～AC-008 是唯一计划与验收真值。2026-08-21 已完成集中 QA：定向前端 11/11、api-server 3/3、control-plane 2/2、Rust 静态门禁与桌面/移动受保护页面取证通过；仅保留仓库既有 TypeScript build 错误与 140 条 i18n unused-key warning 作为非本次阻断风险。用户验收反馈后，Props、限制、示例和上游来源已进一步收敛为单列字段流（提交 `b8b6309b`），定向前端 6/6 与桌面/移动运行态均通过。
