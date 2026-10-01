---
memory_type: feedback
feedback_category: interaction
topic: menu-scroll-experience-needs-visual-evidence
summary: 菜单滚动需要浏览器视觉与真实 DOM 证据；修复时保留用户明确设计的 60vh 高度，并确认嵌套弹层只有一个滚动 owner。
keywords:
  - 菜单滚动
  - 滑轮
  - 跳页感
  - 截图
match_when:
  - 用户反馈下拉菜单滚动卡顿或突然跳动
  - 自动化测试显示 scrollTop 正常而用户仍指出体验问题
created_at: 2026-09-26 11
updated_at: 2026-10-01 18
last_verified_at: 2026-10-01 18
decision_policy: direct_reference
scope:
  - frontend verification
  - user interaction
---

# 菜单滚动体验需要视觉证据

## 规则

- 不以单次 `scrollTop` 增加或 API 请求数不变，直接否定用户报告的卡顿、跳页感或闪现。
- 结合用户截图中的视口、滚动条和末项可见性，核对菜单高度、每格滑轮位移、边界反馈和视觉连续性。
- 设置下拉抽屉的 `60vh` 高度是用户明确的视觉设计约束；优化滚动时不能擅自扩展到接近整屏，应在该高度内修复滚动容器。
- 嵌套菜单出现双滚动条时，不能只用 CSS 选择器测试判定修复；用浏览器检查弹层外层与菜单内层的计算样式、可滚动高度和实际滚动位置。

## 原因

本次设置菜单在自动化中按 120px 增量正常滚动，但用户随后提供截图，明确指出接近底部时的卡住与跳动体验；数据正确不能替代体验结论。Agent 擅自把 `60vh` 改成接近整屏后，用户明确纠正这是有意设计的高度。

2026-10-01 顶部树状菜单中，选择器测试通过后用户仍看到双滚动条。Playwright 显示 Ant Design 弹层外层 `DIV` 与内层 `UL` 都有 `overflow-y: auto`；实际 DOM 证据才定位了重复滚动 owner。

## 适用场景

- 下拉菜单、弹层或嵌套容器的滑轮反馈排查。
