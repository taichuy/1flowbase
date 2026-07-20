---
memory_type: project
topic: Frontstage scoped page tree 父级语义
summary: FrontStagePage 使用当前 rootNode.children 的局部树语义；路由装配层负责把局部 parentId null 转换为后端真实 rootNode.id。
keywords:
  - frontstage
  - page tree
  - scoped root
  - parent_id
  - drag move
match_when:
  - 调整 Frontstage 页面树创建、拖拽、移动到未分组或 scoped topbar root
created_at: 2026-07-20 09
updated_at: 2026-07-20 09
last_verified_at: 2026-07-20 09
decision_policy: verify_before_decision
status: active
scope:
  - web/app/src/app/router.tsx
  - web/app/src/features/frontstage/pages/FrontStagePage.tsx
  - web/app/src/features/frontstage/_tests/frontstage-root-routing.test.tsx
---

# Frontstage scoped page tree 父级语义

- 谁在做什么：FrontStagePage 只操作当前 topbar group 的 `rootNode.children`；组件内 `parentId: null` 表示当前 scope 内顶层。
- 为什么这样做：页面组件不可见全局导航树，也不应持有后端绝对父级 ID。
- 为什么要做：后端 `parent_id: null` 表示全局根；若路由直接透传局部 `null`，页面会被移出当前 topbar group，并在 refetch 后从侧栏消失。
- 截止日期：2026-07-20 已实现并完成真实拖拽、刷新和清理验收。

冻结规则：

- `FrontStageWorkspaceContent` 是局部父级到绝对父级的转换 owner。
- 当前 rootNode 为 group 时，创建或移动请求中的局部 `parentId: null` 必须转换为 `rootNode.id`。
- 非空父级 ID 原样透传；无 scoped group root 时，真实 `null` 保持不变。
- FrontStagePage、拖拽组件和后端 API contract 不增加兼容别名或全局 root 知识。
