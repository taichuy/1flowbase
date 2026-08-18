---
memory_type: project
topic: 通用可调整抽屉按页面路径记忆宽度
summary: 用户于 2026-08-18 09 确认：ResizableDrawer 默认以当前 pathname 作为浏览器本地宽度偏好的作用域；同页复用抽屉共享最后一次宽度，不向调用方暴露 storageKey。
keywords:
  - resizable-drawer
  - drawer-width
  - localStorage
  - pathname
  - settings
match_when:
  - 需要调整 ResizableDrawer 的宽度、持久化或作用域策略
  - 需要新增可调整抽屉并判断其宽度是否应单独配置
  - 需要讨论同页面不同抽屉是否共享宽度偏好
created_at: 2026-08-18 09
updated_at: 2026-08-18 09
last_verified_at: 2026-08-18 09
decision_policy: verify_before_decision
scope:
  - web/app/src/shared/ui/resizable-drawer
---

# 通用可调整抽屉按页面路径记忆宽度

## 时间

`2026-08-18 09`

## 谁在做什么

用户确认交互语义；AI 在 `ResizableDrawer` 内实现路径作用域的本地宽度恢复，并补充自动化验证。

## 为什么这样做

抽屉关闭时会销毁组件，本地 React state 无法保留用户已经调整的工作宽度。宽度是页面级体验偏好，不是后端业务数据。

## 为什么要做

同一路径的可调整抽屉在重新打开或刷新后继续使用最后宽度，且调用方无须声明或维护存储 key。

## 截止日期

未指定；本轮实现已完成，等待用户验收。

## 决策背后动机

以 `window.location.pathname` 生成 `localStorage` key，查询参数和 hash 不参与作用域。同页面的不同抽屉有意共享最后宽度；读取值必须是有限数字，并按当前抽屉的宽度边界裁剪。浏览器存储不可用时，抽屉仍使用本地状态正常调整。

## 关联文档

- `web/app/src/shared/ui/resizable-drawer/ResizableDrawer.tsx`
- `web/app/src/shared/ui/resizable-drawer/_tests/ResizableDrawer.test.tsx`
