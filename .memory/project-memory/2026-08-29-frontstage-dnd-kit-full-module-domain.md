---
memory_type: project
topic: Frontstage Native Block 全域开放 @dnd-kit
summary: 用户确认 #1929 对宿主已安装且构建器可解析的 @dnd-kit/* package root 与内部子路径不设人为白名单，由构建期 inventory、Runtime loader 与 editor declarations 共享同一真值。
keywords:
  - issue 1929
  - dnd-kit
  - native trusted block
  - module inventory
  - internal subpath
  - ShadowRoot drag
match_when:
  - 实现或验收 #1929
  - 修改 Native Block import policy、module registry 或声明生成
  - 处理 @dnd-kit import denied 或拖拽官方示例
created_at: 2026-08-29 08
updated_at: 2026-08-29 09
last_verified_at: 2026-08-29 09
decision_policy: verify_before_decision
status: user-acceptance
scope:
  - https://github.com/taichuy/1flowbase/issues/1929
  - web/packages/page-runtime
  - web/app/src/features/frontstage/lib/native-modules
  - web/app/vite.config.ts
  - 16cb3a93-516a-4b07-96a2-040e5df7782a
---

# Frontstage Native Block 全域开放 @dnd-kit

## 谁在做什么

当前开发会话已将用户确认的方向冻结为 Single Issue #1929：Native Trusted Block 可导入宿主已安装且当前构建可解析的全部 `@dnd-kit/*` package root 与内部子路径，不再维护手写包名、公开 export 或函数白名单。

## 为什么这样做

真实 Block 使用 Ant Design Tabs 官方拖拽示例，但拖拽能力来自独立 `@dnd-kit` 包。1flowbase 已安装相关依赖，当前失败来自 Runtime Registry 未暴露模块，而不是源码或 AntD Tabs 错误。用户明确接受内部子路径随上游升级变化的兼容风险，要求由宿主升级测试承担，而不是继续限制作者能力。

## 为什么要做

目标是让官方拖拽示例与后续 `@dnd-kit` 能力无需逐包、逐函数补白名单即可运行，同时仍限定在宿主实际安装与可解析的依赖，不扩大到其他 npm scope 或运行时联网安装。

## 实现与验收状态

无日历截止日期。#1929 已完成实现并进入 `phase:user-acceptance`：构建期 inventory 只扫描 `web/app/node_modules/@dnd-kit`，compiler、Runtime loader 和 editor declarations 共享同一清单，模块按 import 懒加载。

验收证据：定向 Vitest 8 个文件 / 48 项全通过；TypeScript、ESLint、Prettier、diff check 和生产 Vite build 通过；真实 Block `16cb3a93-516a-4b07-96a2-040e5df7782a` 在 ShadowRoot 中完成 pointer drag，Tabs 顺序从 `Tab 1|Tab 2|Tab 3` 变为 `Tab 2|Tab 3|Tab 1`，双页面实例状态互不污染，console/page errors 均为 0。保留风险是内部子路径可随上游升级变化，按已批准 contract 由升级时构建和回归暴露。
