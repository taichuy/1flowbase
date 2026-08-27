---
memory_type: project
topic: Frontstage Native Block Affix surface adapter
summary: 用户确认 #1914 采用 surface-owned Affix；实现已推送 dev 并更新指定 Block，当前等待用户验收固定顶部与 Anchor 偏移效果。
keywords:
  - issue 1914
  - native affix
  - surface portal
  - anchor targetOffset
  - ShadowRoot
  - RGL transform
match_when:
  - 实现或验收 #1914
  - 修改 Native Block Affix、Anchor offset 或 surface overlay
  - 处理 position fixed 被 RGL transform 或 contain 截断
created_at: 2026-08-28 00
updated_at: 2026-08-28 00
last_verified_at: 2026-08-28 00
decision_policy: verify_before_decision
status: active
scope:
  - https://github.com/taichuy/1flowbase/issues/1914
  - web/app/src/features/frontstage/lib/native-modules/native-affix-runtime.tsx
  - web/app/src/features/frontstage/lib/native-modules/native-affix-layer.ts
  - 01a043f6-fe2c-7822-99d9-eb5fa50d9feb
---

# Frontstage Native Affix 等待用户验收

## 谁在做什么

当前开发会话已为 Native React Block 增加 surface-owned `Affix` adapter，并将指定 Block 从裸 `position: fixed` 改为 Ant Design `Affix`；代码提交为 `58d98fbad`。

## 为什么这样做

RGL transform、ShadowRoot 外层和设计模式 paint containment 会改变或裁剪普通 fixed DOM。真实 scroll owner 才能观察并控制吸附生命周期，因此 Portal 负责离开 transform，原生 sticky 负责 Block 边界，三态状态机只发布稳定的 `onChange`。

## 为什么要做

用户希望官方 Anchor targetOffset 示例在 Frontstage 中保持固定顶部，并让 Part 1/2/3 的落点位于 Header 下方，而不是被遮挡。

## 验收候选与下一事件

无日历截止日期。浏览器证据显示 Header 与 owner 顶部一致，点击 Part 2 后 target 与 Header bottom 误差为 0px，且无页面错误；#1914 当前为 `phase:user-acceptance`，下一事件是用户刷新指定 Block 检查视觉，确认后关闭 Issue。
