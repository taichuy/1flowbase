---
memory_type: feedback
feedback_category: repository
topic: Drawer 实时 resize 必须以组件原生拖拽状态接管 motion
summary: 对 Drawer 实时改宽度前先检查底层 wrapper 的 transition；不得把持续追帧的缓动误诊为单纯布局重排，并应优先复用已安装组件的原生 resizable 能力。
keywords:
  - drawer
  - resize
  - transition
  - reflow
  - antd
  - rc-drawer
match_when:
  - 实现或诊断侧边抽屉、Inspector、面板的拖拽 resize
  - 使用 Ant Design Drawer 并在交互期间改变 width 或 height
created_at: 2026-08-08 00
updated_at: 2026-08-08 00
last_verified_at: 2026-08-08 00
decision_policy: direct_reference
scope:
  - web/app/src/shared/ui/resizable-drawer
---

# Drawer resize 必须由原生拖拽状态关闭 wrapper motion

## 规则

- 实时拖动 width/height 时，必须先检查底层 Drawer wrapper 是否保留 opening/closing transition；若保留，连续更新目标尺寸会让边缘持续滞后于指针。
- 当前锁定的 Ant Design 6.5.3 已提供 `Drawer resizable`；优先复用其 drag state（拖动时 `transition: none`、`will-change` 与内容 pointer-event 管控），不要再以 document query + 内联 width 写入复刻同一机制。
- 实际尺寸改变仍必然触发布局；只有需要“预览线、松手后才变更内容尺寸”的产品语义时，才使用 Splitter 的 lazy / ghost-preview 路径。

## 原因

Drawer 的开关动画与实时 resize 是两种不同的 motion 语义。前者需要缓动，后者需要边缘与指针同步；混用会产生明显橡皮筋感。

## 适用场景

共享右侧抽屉、表单配置抽屉，以及未来的可调整 Inspector 宽度。
