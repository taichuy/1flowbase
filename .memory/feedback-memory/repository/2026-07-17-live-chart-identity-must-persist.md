---
memory_type: feedback
feedback_category: repository
topic: live-chart-identity-and-count-must-persist
summary: 自动刷新的实时图表中，进程身份与当前数量属于必要上下文，必须在图表工具栏、图例或摘要区常驻，不能只放在会因轮询刷新而消失的 tooltip 里。
keywords:
  - live-chart
  - tooltip
  - process-memory
  - persistent-context
match_when:
  - 调整实时资源监控或自动刷新图表
  - 展示进程、运行目标、系列身份或当前数量
created_at: 2026-07-17 22
updated_at: 2026-07-17 22
last_verified_at: 2026-07-17 22
decision_policy: direct_reference
scope:
  - web/app/src/features/settings/components/SystemRuntimePanel.tsx
  - web/app/src/features/settings/components/system-runtime/RuntimeMetricsChart.tsx
---

# Live Chart Identity And Count Must Persist

规则：在 2 秒轮询等自动刷新图表中，运行目标、系列身份和当前进程数是理解曲线的必要上下文，应当常驻展示。

原因：Tooltip 只适合查看某个历史时点；采样刷新会让它消失，用户来不及读取进程身份和数量。

适用场景：实时资源监控、应用监控、自动轮询的 ECharts 折线图。Tooltip 继续承载历史时点值，工具栏、图例或摘要区承载当前身份与数量。
