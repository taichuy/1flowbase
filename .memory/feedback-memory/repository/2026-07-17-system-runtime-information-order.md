---
memory_type: feedback
feedback_category: repository
topic: System Runtime 信息顺序与运行目标选择归属
summary: /settings/system-runtime 的信息层级固定为运行概览、运行环境、资源监控；运行目标属于运行环境；运行环境只展示进程内存和两个路径，不展示 locale 元信息。
keywords:
  - system-runtime
  - runtime-environment
  - resource-monitoring
  - target-selector
  - information-order
  - locale-metadata
match_when:
  - 调整 System Runtime 页面 section 顺序
  - 放置 API Server / Plugin Runner 选择器
  - 设计运行环境与资源监控的信息归属
  - 调整运行环境详情字段与响应式布局
created_at: 2026-07-17 14
updated_at: 2026-07-17 15
last_verified_at: 2026-07-17 15
decision_policy: direct_reference
scope:
  - web/app/src/features/settings/components/SystemRuntimePanel.tsx
---

# System Runtime 信息顺序与运行目标选择归属

## 规则

页面顺序使用“运行概览 → 运行环境 → 资源监控”。API Server / Plugin Runner 下拉放在运行环境标题行；资源监控标题只保留采集状态与时间窗口。运行环境不展示当前语言、回退语言、支持语言，只保留进程内存、插件安装路径、宿主扩展路径；详情桌面三列、窄屏单列。

## 原因

运行目标决定当前查看的是哪个运行环境，并同时影响进程内存与下方监控序列。把选择器放在资源监控里会让运行环境离概览过远，也会误导用户认为目标只影响图表。locale 元信息对当前诊断没有实际价值，保留会稀释进程与安装路径信息，并增加无效纵向占用。

## 适用场景

继续调整 System Runtime 页面层级、目标切换位置、环境详情或监控标题行时。
