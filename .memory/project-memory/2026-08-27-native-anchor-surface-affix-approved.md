---
memory_type: project
topic: Native Anchor Affix 生命周期改由 Frontstage surface 拥有
summary: 用户于 2026-08-27 确认并已实现 #1910 surface-owned AffixLayer；candidate a89ee2b2a 已推送 dev 并通过 Dev Acceptance，当前等待用户页面验收。
keywords:
  - issue 1910
  - Native Anchor
  - AffixLayer
  - surface ownership
  - ShadowRoot
  - RGL transform
match_when:
  - 实现或验收 #1910 AC-009～012
  - 修改 Native Anchor Affix、surface overlay 或多个 Anchor 接管规则
  - 判断 Anchor target、scroll owner 与视觉 Affix 的所有权
created_at: 2026-08-27 23
updated_at: 2026-08-27 23
last_verified_at: 2026-08-27 23
decision_policy: verify_before_decision
status: active
scope:
  - https://github.com/taichuy/1flowbase/issues/1910
  - web/app/src/features/frontstage/lib/native-modules/native-anchor-runtime.tsx
  - web/app/src/features/frontstage/lib/native-modules/native-block-surface-context.ts
---

# Native Anchor Surface Affix 方向获批

## 谁在做什么

当前开发会话以 #1910 为唯一 Single Issue，已把 Native Anchor 的视觉吸附从 Block 内 CSS containing block 迁移到 Frontstage surface-owned AffixLayer。AC-009～012、影响回归、浏览器全范围取证和 Native React fast receipt 已通过。

## 为什么这样做

真实页面证明 CSS sticky 几何、左右 Col 高度和 active 公式都正确，但 Anchor 离开所属 Block 后被 containing block 推出视口；可见页面已经进入后续 Block。问题是视觉生命周期 owner 错置，不是阈值、样式丢失或 active 算法错误。

## 为什么要做

用户期望 Ant Design 官方 Affix 语义：Anchor pinned 后可以跨过所属 Block bottom，在 Frontstage surface 生命周期内继续可见；同时必须保留 ShadowRoot target 隔离和禁止非点击页面回拉的合同。

## 硬边界

- `targetRoot` 拥有目标隔离，`scrollOwner` 拥有滚动与 active，AffixLayer 拥有视觉吸附。
- 不修改用户 Block 源码或 Ant Design 上游，不 monkeypatch 全局 DOM，不恢复逐帧 scroll geometry 写入。
- 当前只做 Native Anchor 内部 surface layer，不泛化为公共 Overlay Coordinator。
- 多 Anchor 使用最近进入者接管，反向滚动恢复前者，视觉不重叠。

## 截止与下一事件

无日历截止日期。candidate `a89ee2b2a` 已推送 `dev`，#1910 为 `phase:user-acceptance`；下一事件是用户在指定页面完成手动验收，确认后关闭 Issue。
