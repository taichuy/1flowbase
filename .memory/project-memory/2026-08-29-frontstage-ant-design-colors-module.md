---
memory_type: project
topic: Frontstage Native Block @ant-design/colors 模块合同
summary: 用户确认 #1932 将 @ant-design/colors 作为直接宿主依赖和 lazy Runtime module，仅开放包根公开导出；实现已通过目标 ColorPicker 浏览器验收，等待最终用户验收。
keywords:
  - issue 1932
  - native trusted block
  - ant-design colors
  - ColorPicker
  - lazy module
  - module registry
match_when:
  - 实现或验收 #1932
  - 修改 Native Block module registry、颜色模块或兼容清单
  - 处理 @ant-design/colors import denied
created_at: 2026-08-29 11
updated_at: 2026-08-29 11
last_verified_at: 2026-08-29 11
decision_policy: verify_before_decision
status: user-acceptance
scope:
  - https://github.com/taichuy/1flowbase/issues/1932
  - web/app/src/features/frontstage/lib/native-modules
  - web/app/src/features/frontstage/lib/native-trusted-block-runtime-compatibility.ts
  - 0a782e38-02f5-4536-b807-a9d98fc15d00
---

# Frontstage @ant-design/colors 等待用户验收

## 谁在做什么

当前开发会话已将 `@ant-design/colors` 提升为 `web/app` 直接依赖，并通过 Native Module Registry 的独立 lazy loader 开放包根公开运行时导出；真实包声明进入 Monaco，兼容清单以 `lazyModules` 记录宿主版本合同。

## 为什么这样做

目标 ColorPicker 官方示例依赖 `generate`、标准色板和 `presetPalettes`。复制颜色数据或算法会丢失主题响应并产生上游版本漂移；自动开放全部 `@ant-design/*` 又会扩大升级风险，因此由 Runtime 明确拥有这个单包边界。

## 为什么要做与验收状态

用户希望 Block `0a782e38-02f5-4536-b807-a9d98fc15d00` 保持官方源码即可运行。无日历截止日期；TDD 红灯确认原编译拒绝，定向测试与 TypeScript/ESLint 已通过，真实浏览器显示两个 ColorPicker trigger，预设 `primary` 面板进入 Top Layer，page/console error 均为 0。#1932 等待用户刷新页面验收后关闭。
