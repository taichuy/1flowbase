---
memory_type: project
topic: Frontstage Native Block dayjs 根模块合同
summary: 用户确认 #1933 将 dayjs 作为直接宿主依赖和 lazy Runtime module，仅开放包根默认导出；实现已通过目标 DatePicker 浏览器验收，等待最终用户验收。
keywords:
  - issue 1933
  - native trusted block
  - dayjs
  - DatePicker
  - lazy module
  - module registry
match_when:
  - 实现或验收 #1933
  - 修改 Native Block module registry、日期模块或兼容清单
  - 处理 dayjs import denied
created_at: 2026-08-29 13
updated_at: 2026-08-29 13
last_verified_at: 2026-08-29 13
decision_policy: verify_before_decision
status: user-acceptance
scope:
  - https://github.com/taichuy/1flowbase/issues/1933
  - web/app/src/features/frontstage/lib/native-modules
  - web/app/src/features/frontstage/lib/native-trusted-block-runtime-compatibility.ts
  - ffe4026e-2dab-4c27-8804-b8c34072513a
---

# Frontstage dayjs 等待用户验收

## 谁在做什么

当前开发会话已将 `dayjs` 提升为 `web/app` 直接依赖，并通过 Native Module Registry 的独立 lazy loader 开放包根默认导出；真实包声明进入 Monaco，兼容清单以 `lazyModules` 记录宿主版本合同。

## 为什么这样做

目标 DatePicker 定制面板需要创建、增减、比较和格式化日期。依赖 Ant Design 的传递依赖会让宿主合同随上游内部实现漂移；自动开放全部插件和 locale 又会扩大运行时与升级兼容面，因此只由 Runtime 拥有 `dayjs` 包根边界。

## 为什么要做与验收状态

用户希望 Block `ffe4026e-2dab-4c27-8804-b8c34072513a` 保持现有源码即可运行。无日历截止日期；TDD 红灯确认 `web/app` 原先无法直接解析 dayjs，定向测试、TypeScript、ESLint 与真实浏览器目标 Block 验收已通过。`dayjs/plugin/*` 与 `dayjs/locale/*` 仍按已确认边界拒绝。#1933 等待用户刷新页面验收后关闭。
