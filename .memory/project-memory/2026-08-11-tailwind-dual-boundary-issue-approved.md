---
memory_type: project
topic: Tailwind CSS 低代码与主仓双边界已批准
summary: 用户已批准 issue #1671：低代码区块直接开放标准 Tailwind 并由每区块 ShadowRoot 隔离；主仓源码禁止全局 utilities，只允许 CSS Modules + @apply，并以静态门禁和 style-boundary 分别治理扩散与回归。
keywords:
  - Tailwind CSS
  - low-code block
  - Shadow DOM
  - CSS Modules
  - style-boundary
  - Preflight
match_when:
  - 实现或调整低代码区块 Tailwind CSS 能力
  - 判断主仓源码是否允许 Tailwind utility
  - 修改 Tailwind、CSS Modules、Preflight 或样式扩散门禁
created_at: 2026-08-11 23
updated_at: 2026-08-11 23
last_verified_at: 2026-08-11 23
decision_policy: verify_before_decision
status: ready
source_issue: "#1671"
scope:
  - web/packages/page-runtime
  - web/app/src/features/frontstage
  - web/app/src/style-boundary
  - web/app/src/styles
  - web/packages/ui
  - web/scripts
  - scripts/node/check-style-boundary
---

# Tailwind CSS 低代码与主仓双边界

## 谁在做什么

1flowbase 将按 Single Issue #1671 引入 Tailwind CSS：低代码 Native React 区块直接使用标准 `tailwindcss` 包名和标准 utility class；主仓普通前端源码如需 Tailwind，只能通过就近 `CSS Modules + @apply` 生成组件级 selector。

## 为什么这样做

模型对标准 Tailwind 有稳定训练先验，不应暴露私有样式包名；同时主仓普通组件运行在 Light DOM，全局 utility selector 无法满足组件隔离要求。低代码已有每区块独立 ShadowRoot，主仓则需要 CSS Modules 承担 selector 隔离。

## 为什么要做

目标是提高 AI 生成区块的样式质量，同时阻止 Tailwind、Preflight 或未来 AI 生成的全局样式污染 Ant Design、其他组件和页面。

## 截止日期

未指定。Issue #1671 已进入 `phase:ready`，尚未开始实现。

## 已确认决策

- 低代码公开标准 `import 'tailwindcss'` 与标准 class，不使用模型未知的私有包名或私有前缀。
- 低代码 Tailwind 由主仓官方 catalog/asset build 发布，不依赖 External npm Pack。
- Tailwind 只提供 theme/utilities，关闭 Preflight，并作为 `shadow_style` 注入实际 import 它的单个区块 ShadowRoot。
- 主仓禁止全局 Tailwind utilities、Preflight 和 TSX 直接使用全局 utility class。
- 主仓 Tailwind authoring 只允许就近 `*.module.css + @apply`；CSS Modules 负责阻止跨组件。
- 静态门禁负责阻止全局导入和非法产物；style-boundary 只负责可观察视觉与布局回归，不替代 selector 隔离。
- Tailwind 生成资产不得自定义 `.ant-*` 覆盖；该限制不适用于 Ant Design 自身 CSS-in-JS 规则。
- 扩大到全量 class、运行时编译、启用 Preflight、主仓 JSX 全局 utility 或迁移既有样式体系时返回 discussion。

## 在线真值

- GitHub Single Issue：#1671。
- 分级与阶段：`plan:single`、`grade:g3`、`phase:ready`。
- 验收账本：AC-001 至 AC-008，覆盖低代码标准 API、ShadowRoot A/B/host 隔离、无 Preflight、class inventory、Ant Design 基线、主仓全局导入阻断、CSS Module sibling 隔离和 component style-boundary 执行。
