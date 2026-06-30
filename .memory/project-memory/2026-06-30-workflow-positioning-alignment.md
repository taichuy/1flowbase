---
memory_type: project
title: Workflow positioning alignment pending
created_at: 2026-06-30 17
updated_at: 2026-06-30 17
decision_policy: verify_before_decision
status: alignment_pending
scope:
  - api/apps/api-server
  - api/crates/control-plane
  - api/crates/domain
  - api/crates/storage-durable
  - web/app/src/features/agent-flow
  - web/app/src/features/applications
keywords:
  - workflow
  - agent-flow
  - trigger
  - scheduler
  - custom-api
  - api/ex
  - openapi
  - logs
  - monitoring
---

# Workflow Positioning Alignment Pending

## 谁在做什么

用户在 `2026-06-30 17` 提出开始做 workflow，定位为系统工作流的内置工作补充。期望它支持同步和异步两种运行模式，支持定时任务、API 触发、自定义 `api/ex/...` 接口触发并进入 OpenAPI 文档，同时应用侧布局参考 agent-flow，但侧边栏只保留工作流、日志、监控三类。

## 为什么这样做

当前 agent-flow 已比较稳定；workflow 需要在已有 flow/runtime/public API 能力之上补齐更通用的触发和运行入口，而不是只继续服务模型兼容 API。

## 为什么要做

工作流将成为应用内部自动化和系统工作补充入口：定时触发负责异步任务，API 触发可同步或异步执行，同步模式需要自定义返回值，自定义路径需要成为可发现、可文档化的接口能力。

## 截止日期

无固定截止日期；当前只处于 problem-framing alignment，尚未确认 L1/L2/L3 方案。

## 当前待确认

候选方向需要围绕“复用现有 flow/runtime/public API 真值层”还是“创建独立 workflow 领域”拍板。进入实现前必须先完成 issue gate；涉及后端 API、调度、OpenAPI、运行记录和前端工作台时，后续需要按 `test-driven-development` 和 `qa-evaluation` 规则验收。
