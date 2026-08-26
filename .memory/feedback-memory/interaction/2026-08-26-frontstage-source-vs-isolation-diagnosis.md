---
feedback_category: interaction
decision_policy: direct_reference
date: 2026-08-26 10
---

# 先定位用户 Block 源码问题，再讨论平台隔离

规则：当用户询问低代码 Block 的显示异常时，先明确区分 Block 源码的布局约束与平台隔离层，并用最小 CSS 因果说明哪一侧导致现象。

原因：用户要的是能直接修改的根因；把“隔离边界”泛化为唯一问题，会掩盖如固定/绝对定位元素不参与自动高度、源码自身 `overflow: hidden` 裁切等更直接的原因。

适用场景：Frontstage TSX Block 的视觉差异、浮层、固定定位元素、官方示例对比与线上 MCP 调整。
