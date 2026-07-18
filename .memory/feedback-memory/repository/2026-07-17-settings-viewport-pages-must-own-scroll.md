---
memory_type: feedback
feedback_category: repository
topic: Settings viewport 页面必须由 section body 接管纵向滚动
summary: SettingsRouteShell 使用固定 viewport 高度和 overflow hidden；内容可能超过视口的 section 必须使用 SettingsSectionSurface fill 模式，让 body overflow auto，不能使用 auto surface 后把超出内容直接裁掉。
keywords:
  - settings
  - viewport
  - scroll
  - overflow
  - SettingsSectionSurface
match_when:
  - 修改 settings 下可能超过一屏的页面
  - 调整 SettingsSectionSurface 的 heightMode
  - 页面内容被裁切、没有滚动条或左侧导航需要固定
created_at: 2026-07-17 14
updated_at: 2026-07-17 14
last_verified_at: 2026-07-17 14
decision_policy: direct_reference
scope:
  - web/app/src/features/settings
  - web/app/src/shared/ui/section-page-layout
---

# Settings viewport 页面必须由 section body 接管纵向滚动

## 规则

`SettingsRouteShell` 采用 `heightMode="viewport"` 时，内容可能超过一屏的 section 使用 `SettingsSectionSurface heightMode="fill"`，由 `.settings-section-surface__body` 负责 `overflow: auto`；左侧导航和顶部壳层保持固定。

## 原因

viewport 布局的外层和 content 都是 `overflow: hidden`。若内部 surface 使用默认 `auto`，它不会继承固定高度，也没有滚动容器，超出视口的内容会被裁切，用户看不到滚动条。

## 适用场景

- settings 页面包含长表格、图表、环境信息或多个纵向 section。
- 页面从 `fill` 改为 `auto` 以消除空白时，必须先确认滚动责任是否仍存在。
- 验收应在真实路由测量 `scrollHeight > clientHeight`、`overflow-y: auto` 和非零 `scrollTop`，不能只看独立组件截图。
