---
memory_type: project
topic: 统一异步首载状态与 loading 视觉
summary: Issue #1993 已完成实现与 QA，使用固定英文 thinking 的蓝色动画统一阻塞首载反馈，不接入多语言，并保留框架挂载前原生等价实现、后台已有内容与低代码编译动画。
keywords:
  - loading
  - thinking
  - async state
  - silent blank
  - bootstrap loader
  - issue 1993
match_when:
  - 实现或调整前端应用、路由、页面、关键区域的首次加载反馈
  - 修改 LoadingState、index.html bootstrap loader 或登录页异步加载
  - 判断后台刷新、EmptyState、低代码编译动画是否纳入统一 loading
created_at: 2026-09-05 14
updated_at: 2026-09-05 14
last_verified_at: 2026-09-05 14
decision_policy: verify_before_decision
scope:
  - web/app/index.html
  - web/app/src/shared/ui/loading-state
  - web/app/src/features/auth/pages/SignInPage.tsx
  - frontend async boundaries
---

# 统一异步首载状态与 Loading 视觉

## 时间

`2026-09-05 14`

## 谁在做什么

用户已确认采用统一异步可用性方向，并要求将现有蓝色圆弧旋转动画与 `thinking` 文案作为阻塞首载的统一视觉。GitHub Issue [#1993](https://github.com/taichuy/1flowbase/issues/1993) 是实现和验收的 Single Issue 真值；实现与 QA 证据已经回挂，Issue 当前为 `phase:user-acceptance`。

## 为什么这样做

现有应用启动和部分路由已使用共享 `LoadingState`，但登录入口发现、必需动态 Block 与部分页面边界仍会在无可用内容时渲染空白或使用散落反馈。统一视觉仍由最近且掌握可用性语义的页面或区域 owner 放置，不能把任意 pending 请求等价为全屏 loading。

## 为什么要做

目标是让“无可用内容且必需依赖仍在加载”的静默空白时间归零，同时保持后台刷新、局部提交和已有内容可交互，避免全局请求遮罩带来的闪烁与错误阻塞。

## 截止日期

无；Issue 当前为 `phase:user-acceptance`，实现与 QA 已完成。

## 决策背后动机

React/Ant Design 可用后以共享 `LoadingState` 为 canonical 实现；`web/app/index.html` 在框架尚未挂载时保留原生 CSS 等价动画。低代码编译现有动画明确保持不变。状态规则区分 initial-loading、refreshing、degraded、failed 与 empty，已有内容刷新时不得清空主体。

## 当前结果

- 共享 `LoadingState` 固定使用 `#1677ff`、48px 指示器与英文 `thinking` 可见/可访问文案；该文案不进入多语言资源。
- 登录入口发现和必需认证 Block 分包加载不再静默空白；Hero 等装饰资源仍可静默降级。
- 阻塞首载边界已迁移；后台刷新保留已有内容，空结果继续进入 EmptyState。
- `index.html` 保留 48px / 1s 原生 loader；低代码编译 loading 路径无行为改动。
- 定向测试、TypeScript/build、i18n hygiene、style-boundary、桌面与移动端冷载取证已完成；组合旧测试仍存在 application i18n 隔离与 Monaco jsdom 历史缺口，不属于本次产品回归。

## 关联文档

- GitHub Issue #1993：`统一异步首载状态与 loading 动画，消除页面静默空白`
- `.memory/project-memory/2026-07-19-schema-ui-local-loading-boundaries.md`
