---
memory_type: project
topic: UI 代码模板复用原生区块运行时预览
summary: 用户确认代码模板编辑器必须复用真实原生区块运行时、workspace 已注册组件和组件 UI；运行依赖从未保存源码静态 import 解析，模板仅以临时区块身份替换真实页面实例。页面绑定的 API、事件、导航和 outputs 仍需真实后端页面会话，不能伪造。
keywords:
  - ui-code-template
  - runtime-preview
  - authoring-block
  - jsx-studio
  - native-react
match_when:
  - 调整代码模板编辑器的预览、运行或上下文语义
  - 判断模板预览是否应调用真实接口或绑定页面实例
  - 排查代码模板与区块实例的运行行为差异
created_at: 2026-08-20 22
updated_at: 2026-08-21 18
last_verified_at: 2026-08-21 18
decision_policy: verify_before_decision
scope:
  - web/app/src/features/settings/components/ui-management/UiCodeTemplateStudio.tsx
  - web/app/src/features/settings/_tests/ui-management/ui-management-code-templates.test.tsx
  - web/app/src/features/frontstage/components/jsx-studio/JsxStudioRunPanel.tsx
---

# UI 代码模板共享原生运行时预览

## 谁在做什么

- 用户确认模板编辑器直接复用真实原生区块运行面板与 Portal UI，不要求先选择真实页面实例。
- AI 于 2026-08-20 完成实现，提交 `d3d466ab7`，并快进合入 `dev`。

## 为什么这样做

模板与真实区块的差异不是组件 UI 或模块权限，而是没有已保存页面实例。继续把 workspace 已注册组件、官方组件或基础 `ctx` 人为阉割，会让默认模板无法预览并形成两套运行语义；但没有真实页面会话时也不能伪造 API 调用、事件、导航或 outputs 的授权归属。

## 已确认边界

- 点击运行时冻结当前未保存草稿；预览不保存模板，也不修改任何页面区块实例。
- 预览使用 `authoringBlock`、当前工作区、当前用户、合成设置页和默认空 props。
- 预览按当前未保存源码的静态 import 解析 workspace 已注册模块并传入共享 `JsxStudioRunPanel`；Catalog 只用于模板与运行时身份，不产生依赖锁或导入限制。
- `ctx.api`、events、navigation、outputs 仍复用受控不可用上下文，直到产品提供真实页面/Tab 后端预览会话；这不是模块或 UI 的受限模式。
- 删除“受限预览”提示，避免把完整共享 UI / 模块运行时误标为阉割版。

## 2026-08-21 源码依赖真值更新

用户明确纠正：区块就是代码，组件目录和模块锁不能被正在编辑区块的 Catalog `code_modules` 限制。插入组件必须写入源码 import，后端从该源码解析注册模块、版本与资产锁；保存、source patch 和预览均使用同一规则。

## 证据与截止

- 定向测试覆盖未保存草稿、合成 block、注册模块依赖锁、基础运行时上下文、Catalog loading 和不保存行为；`7/7` 通过。
- 浏览器实测正式模板的 `@1flowbase/block-sdk`、`@1flowbase/native-components`、`antd` 和 `useState` 能完成渲染与交互。
- i18n hygiene：`0` error，任务 locale 无 warning；生产构建仍被 `dev` 中未改动的 `ModelProviderInstanceDrawer.tsx` 既有 TypeScript 错误阻断。
- 用户接下来进行人工测试；无固定截止日期。
