---
memory_type: feedback
feedback_category: interaction
topic: 助手页面引用必须选择实际元素并在输入框上方展示
summary: 页面引用不得强制提升到最近 div；应捕获鼠标命中的实际 DOM 元素。待发送的多个引用按选择顺序逐行放在 Sender 输入区上方并分别删除，footer 只保留工具与发送操作。
keywords:
  - embedded assistant
  - page reference
  - actual element
  - Sender header
  - composer
  - remove reference
match_when:
  - 修改内置助手的页面元素选择或引用预览
  - 调整聊天 composer 的待发送上下文、附件或引用布局
created_at: 2026-08-15 15
updated_at: 2026-08-15 16
last_verified_at: 2026-08-15 16
decision_policy: direct_reference
status: confirmed
scope:
  - web/app/src/features/agent-flow/components/embedded-assistant
  - web/app/src/features/agent-flow/components/debug-console/conversation
  - api/crates/control-plane/src/application_public_api/run_service.rs
---

# 助手页面引用交互

## 规则

- 点击标题、段落、按钮或内联内容时，引用实际命中的 DOM 元素，不使用 `closest('div')` 自动扩大范围。
- 待发送引用使用 `Sender.header` 内的逐行预览，展示元素摘要、页面来源、字节大小和稳定删除按钮。
- 支持多引用时按用户选择顺序保存和发送，每个引用可分别删除；不用新选择静默替换旧引用。
- 引用预览不与选择工具、模型控件和发送按钮挤在 footer。
- 已发送引用属于消息历史，不提供单独删除，避免展示历史与模型实际上下文失配。

## 原因

最近 `div` 会把细粒度内容扩大成整个页面区块；紧凑 footer Tag 使删除入口容易被压缩或忽略。输入框上方的独立引用行更符合待发送上下文的用户心智。

## 适用场景

内置助手页面引用、聊天附件预览、composer header/footer 责任划分与引用删除交互。
