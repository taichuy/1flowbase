---
memory_type: feedback
feedback_category: repository
topic: Frontstage 代码哈希是区块冷渲染身份
summary: 用户明确 Frontstage TSX 区块以源码内容哈希作为冷渲染身份；页面、路由或 Tab 切出切回不是刷新事件，同一实例代码哈希未变时不得重新创建 Worker、重新编译或重新执行。接口关联、Page/Tab 变量注册等有效依赖必须显式进入源码或由源码显式注册，不能依赖页面 remount 隐式生效。
keywords:
  - frontstage
  - code hash
  - content-addressed cache
  - render identity
  - worker lifecycle
  - page navigation
  - explicit dependency
match_when:
  - 调整 Frontstage 区块运行缓存、Worker 生命周期或页面切换行为
  - 设计代码哈希、编译缓存、渲染快照或运行失效规则
  - 调整 Page/Tab 变量注入、接口连接器或源码依赖边界
created_at: 2026-07-21 23
updated_at: 2026-07-22 03
last_verified_at: 2026-07-22 03
decision_policy: direct_reference
scope:
  - api/apps/api-server/src/routes/frontstage
  - web/packages/page-runtime
  - web/app/src/features/frontstage
---

# Frontstage 代码哈希渲染身份规则

## 规则

- 持久化源码是区块程序真值；同一实例的源码内容哈希未变化时，页面、路由或 Tab 切出切回只恢复已有渲染产物，不重新创建 Worker、编译源码或执行 `main`。
- 导航不是刷新或重新验证信号。需要响应 Page/Tab Signal、接口结果或其他运行输入时，必须由源码中的显式注册机制驱动；不能用组件 remount 兜底。
- 接口连接器、变量注册和其他关联能力只生成或注入可见源码，不建立与源码并行的隐藏 binding 状态。
- 安全会话、运行时 ABI 变化和显式重试属于缓存命名空间或控制面失效，不把它们伪装成源码变化。

## 原因

所有能影响区块能力和关联关系的内容都应在源码中可见；代码未变化时由路由生命周期触发冷执行，会重复加载 Worker 模块、编译和执行，并让页面切换错误地承担刷新语义。

## 适用场景

Frontstage TSX 区块源码读取、内容哈希、运行时 Session、Worker 调度、渲染快照缓存、Page/Tab Signal 和页面导航恢复。
