---
memory_type: feedback
feedback_category: repository
topic: 通用 loading 文案固定英文
summary: 通用蓝色 loading 动画的可见和可访问文案固定为英文 thinking，不为该文案新增或接入多语言资源。
keywords:
  - loading
  - thinking
  - i18n
  - LoadingState
match_when:
  - 修改通用 LoadingState、bootstrap loader 或阻塞首载反馈
  - 判断 loading 文案是否需要本地化
created_at: 2026-09-05 22
updated_at: 2026-09-05 22
last_verified_at: 2026-09-05 22
decision_policy: direct_reference
scope:
  - web/app/src/shared/ui/loading-state
  - web/app/index.html
---

# 通用 Loading 文案固定英文

## 时间

`2026-09-05 22`

## 规则

统一 loading 动画始终显示英文 `thinking`，`role=status` 的 accessible name 同样使用 `thinking`；不要为它增加中文翻译或 i18n key。

## 原因

用户明确认为该短状态词没有翻译必要，固定视觉和文案比跟随 locale 更符合此 loading 契约。

## 适用场景

共享 `LoadingState`、React 挂载前 bootstrap loader，以及采用该共享视觉的所有阻塞首次加载边界。

