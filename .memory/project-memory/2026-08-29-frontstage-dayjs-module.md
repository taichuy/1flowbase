---
memory_type: project
topic: Frontstage Native Block dayjs 模块域合同
summary: 用户将 #1933 扩展为开放已安装 dayjs 包内全部真实 JavaScript 入口；实现使用构建期库存与惰性 loader，已通过插件、locale、DatePicker 浏览器验收，等待最终用户验收。
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
updated_at: 2026-08-29 15
last_verified_at: 2026-08-29 15
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

当前开发会话已将 `dayjs` 提升为 `web/app` 直接依赖，并通过构建期 inventory 自动开放已安装包内全部真实 JavaScript 入口，包括 root、`plugin/*`、`locale/*` 与 `esm/*`；每个入口仍由独立 dynamic import 惰性加载。具有 `.d.ts` 的入口使用真实声明，纯运行时入口使用受控默认导出声明；兼容清单以 `moduleDomains.dayjs` 记录版本和入口数量。

## 为什么这样做

目标 DatePicker 定制面板需要根模块，同页其他官方示例还依赖 `customParseFormat`、`buddhistEra` 等插件。用户明确选择让常见 dayjs 包完整可用，因此 Runtime 以“当前安装包中真实存在的 JavaScript 文件”为 source of truth；未知 `dayjs/*` 仍 fail closed，不退化为任意 npm fallback。

## 为什么要做与验收状态

用户希望 Block `ffe4026e-2dab-4c27-8804-b8c34072513a` 及常见 dayjs 插件/locale 示例保持现有源码即可运行。无日历截止日期；第二轮 TDD 红灯确认构建期库存尚不存在，定向测试、TypeScript、ESLint、production Vite build 与真实浏览器验收已通过。滚动到目标区域后 21 个已挂载 Block runtime error 为 0，原先失败的佛历 locale 示例已渲染。#1933 等待用户刷新页面验收后关闭。
