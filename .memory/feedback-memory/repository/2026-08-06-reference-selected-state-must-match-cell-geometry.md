---
memory_type: feedback
feedback_category: repository
topic: 参考相邻选中态时必须复用整格几何，而非只替换颜色
summary: 用户指定相邻 UI 为选中态参考时，视觉实现必须同时对齐占位尺寸、铺满范围和选中指示，不可仅为原控件添加同色背景。
keywords:
  - selected state
  - reference UI
  - app shell
  - full cell highlight
  - AI assistant
match_when:
  - 用户要求按钮参考相邻页签或菜单的选中态
  - 调整顶栏、导航或工具栏的 active/highlight 样式
created_at: 2026-08-06 16
updated_at: 2026-08-06 16
last_verified_at: 2026-08-06 16
decision_policy: direct_reference
scope:
  - web/app/src/app-shell
  - web/app/src/features/agent-flow/components/embedded-assistant
---

# 参考选中态必须匹配整格几何

## 时间

`2026-08-06 16`

## 规则

用户要求某按钮“参考隔壁 UI 的选中态”时，先核对参考对象的格宽、格高、背景覆盖范围和底部指示；实现应与其同格铺满，不能只给原按钮加一个小尺寸圆角底色。

属于同一模式切换层级的相邻整格入口必须零间距相接；不要让通用操作区间距在两个连续选中态之间露出白缝，后续普通图标入口仍保留原有间距。

## 原因

仅替换颜色会保留控件原有的紧凑边界，视觉层级仍与参考页签不一致，无法表达同一级的固定选中状态。

## 适用场景

顶栏 AI、设计模式、导航页签、工具栏模式切换和其他需要复用相邻选中语法的入口。
