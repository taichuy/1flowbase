---
memory_type: feedback
feedback_category: interaction
topic: Vditor 大纲与编辑器可见性
summary: 用户所说“默认不需要，打开 Vditor 编辑器的大纲”是指不要默认展开 Vditor 左侧大纲面板；完整描述编辑器本身仍应直接显示。
keywords:
  - Vditor
  - outline
  - full_description
  - editor visibility
match_when:
  - 调整 MCP Tool 的完整描述 Vditor
  - 用户提到编辑器大纲、默认开启或左侧面板
created_at: 2026-08-22 13
updated_at: 2026-08-22 13
last_verified_at: 2026-08-22 13
decision_policy: direct_reference
scope:
  - web/app/src/shared/ui/markdown-ir-editor/MarkdownIrEditor.tsx
---

# Vditor 大纲默认关闭

## 规则

- MCP Tool 的 `full_description` 直接展示 Vditor 编辑器。
- 左侧“**大纲**”面板默认关闭，用户可从 Vditor 的更多菜单自行开启。

## 原因

完整描述已经是可选字段，但用户仍需要能直接输入；默认展开的大纲会无意义地占用编辑空间。
