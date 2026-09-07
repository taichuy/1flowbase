---
memory_type: feedback
feedback_category: interaction
topic: 区分打开态模态阻塞与关闭后残留拦截
summary: 诊断浮层导致页面不可用时，必须按时间状态拆分打开态与关闭态；不得在只有关闭后点击失效证据时改变打开态几何或模态语义。
keywords:
  - Drawer
  - overlay
  - modal
  - pointer interception
  - close motion
match_when:
  - 用户报告 Drawer、Modal 或其他全屏浮层导致页面无法点击
  - 浮层关闭后触发器不能再次点击
created_at: 2026-09-07 17
updated_at: 2026-09-07 17
last_verified_at: 2026-09-07 17
decision_policy: direct_reference
scope:
  - frontend interaction diagnosis
  - Native Trusted Block overlay runtime
---

# 区分打开态模态阻塞与关闭后残留拦截

## 规则

- 先按时间状态取证：打开中、关闭动画中、关闭完成后，分别检查视觉、模态语义和 pointer interception。
- “页面无法使用”若只发生在关闭完成后，只修复残留 overlay / Top Layer 的生命周期；不得据此把打开中的全局 Drawer 改成 Block 内局部面板。
- 除非证据明确指出打开态的范围或模态性错误，否则保留组件原有几何、遮罩、焦点与 `aria-modal` 语义。

## 原因

打开态阻塞背景交互是标准模态 Drawer 的预期行为，关闭后仍拦截点击才是缺陷。把两者合并诊断会改变正确的产品语义，并制造比原问题更大的视觉与交互回归。

## 适用场景

Ant Design Drawer / Modal、Popover Top Layer、ShadowRoot portal host，以及任何带关闭动画并在关闭后保留 DOM 的浮层组件。
