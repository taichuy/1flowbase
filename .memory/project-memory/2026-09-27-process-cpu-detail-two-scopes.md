---
memory_type: project
topic: process-cpu-detail-two-scopes
summary: 用户确认系统运行页只在进程详情同时展示 CPU(整机) 与 CPU 单核口径；顶部监控、曲线、进程列表和进程树不增加指标。
keywords:
  - system-runtime
  - process-cpu
  - single-core
match_when:
  - 继续调整系统运行页的进程 CPU 展示或进程采样接口
created_at: 2026-09-27 07
updated_at: 2026-09-27 07
last_verified_at: 2026-09-27 07
decision_policy: verify_before_decision
scope:
  - api/crates/runtime-profile/src/processes.rs
  - api/apps/api-server/src/routes/settings/system.rs
  - web/app/src/features/settings/components/SystemRuntimePanel.tsx
---

# 进程详情 CPU 双口径

用户以系统运行页截图中的进程 CPU 占用为例，要求保留原数值作为 `CPU(整机)`，再增加以一个逻辑核为 100% 的 `CPU`。用户随后将范围明确收窄到进程详情；顶部监控、CPU 曲线、进程列表和进程树维持现状。后端负责从同一进程采样返回两种口径，单核口径可超过 100%。这样做是为了在排查具体进程时直接看到其相对单核的实际负载，同时不扩大监控页的信息密度。无固定截止日期。
