---
memory_type: feedback
feedback_category: repository
topic: Rich Text Catalog 应开放统一完整的 Vditor 编辑体验
summary: 低代码 Rich Text Catalog 应以单一 Vditor 组件提供完整编辑体验，并通过受治理的 BlockContext API 直接上传到后端解析的默认文件表；不把原始 Vditor 构造器和危险 options 交给区块。
keywords:
  - rich-text
  - Vditor
  - MarkdownEditor
  - MarkdownPreview
  - Catalog
  - low-code
match_when:
  - 设计或调整 @1flowbase/rich-text 的 Catalog 公共契约
  - 在低代码区块展示或生成富文本编辑器
  - 决定 Vditor 编辑与预览能力如何开放
created_at: 2026-08-13 09
updated_at: 2026-08-13 10
last_verified_at: 2026-08-13 09
decision_policy: direct_reference
scope:
  - web/packages/rich-text
  - api/plugins/capability-plugins/1flowbase
  - Native React Runtime Catalog
---

# Rich Text Catalog 开放统一 Vditor 体验

## 时间

`2026-08-13 09`

## 规则

面向低代码区块的 Rich Text 主入口应是一个统一的 Vditor 编辑组件，由 Vditor 自己承载编辑模式、分屏 / 预览、工具栏和相关交互；不要要求区块同时编排 `MarkdownEditor` 与 `MarkdownPreview` 并同步两套 UI。

## 原因

Vditor 本身已经拥有编辑、分屏预览、独立预览、编辑模式切换、全屏和大纲等能力。拆成两个并列公共组件会裁掉原生能力，并把状态同步和体验组合复杂度泄漏给低代码区块作者。

## 适用场景

调整 `@1flowbase/rich-text` 的公开 exports、Catalog component metadata、低代码 Demo 或 Vditor React 生命周期封装时适用。上传应复用 `ctx.api` 的 callable interface 和默认文件管理表，由后端解析实际文件表、存储与权限；不要让区块自己查管理端表列表或传环境 UUID。外部 CDN、任意上传 URL、Vditor cache 等能间接调用被 Runtime 禁止能力的 options 不对区块开放。
