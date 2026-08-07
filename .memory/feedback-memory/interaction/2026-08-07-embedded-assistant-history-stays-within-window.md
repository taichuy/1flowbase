---
memory_type: feedback
feedback_category: interaction
topic: 浮动助手的历史会话必须属于同一窗口
summary: 对浮动助手打开历史会话时，禁止把面板相对浏览器视口外置展开；历史应作为同一助手窗口内的可收起侧栏或内层视图，共享窗口高度与视觉边界。
keywords:
  - embedded assistant
  - conversation history
  - floating window
  - sidebar
  - drawer
match_when:
  - 为浮动助手、工作区窗口或 dock 提供历史会话
  - 考虑把历史列表放进 viewport-level Drawer
created_at: 2026-08-07 16
updated_at: 2026-08-07 16
last_verified_at: 2026-08-07 16
decision_policy: direct_reference
scope:
  - web/app/src/features/agent-flow/components/embedded-assistant
---

# 浮动助手的历史会话必须属于同一窗口

## 规则

- 历史会话默认收起；用户点击历史入口后，在助手窗口内部左侧展开固定宽度的侧栏。
- 侧栏与聊天主区共享窗口高度、圆角和层级，不得相对浏览器视口独立定位或超出助手边界。
- 小屏仍保持同一窗口语义：用助手内的全宽历史视图替换聊天主区，而不是浏览器级 Drawer。

## 原因

视口级 Drawer 会与浮动助手的几何和 z-index 脱节，造成高度不一致、明显割裂和内容越界。

## 适用场景

浮动 AI 助手、嵌入式调试控制台及任何带可切换会话列表的工作区窗口。
