---
title: "Swap 容量与使用上限必须分开"
memory_type: feedback
feedback_category: interaction
created_at: "2026-09-22 08"
updated_at: "2026-09-22 08"
decision_policy: direct_reference
status: active
tags: [resources, swap]
---

- 规则：用户要求整机 swap 容量保持 4 GiB，使用目标上限 3 GiB、留出 1 GiB；不得把使用上限解释为缩减交换文件容量。
- 原因：此前误将 swap 文件缩至 3 GiB，用户明确纠正；容量、cgroup 硬限额及 oomd 触发阈值是不同概念。
- 适用场景：本机资源限制及 swap 配置。全局硬限制不可直接配置时，应说明可实现边界，不能用软触发阈值冒充硬上限。

- 历史方案（已被替代）：曾选择 75% oomd 触发线，随后实际终止前端和终端，未满足稳定开发目标。
- 当前确认：4 GiB swap 保留，3 GiB 只观察；90% 紧急兜底仅选择 Rust 组，关闭整个用户会话与 app.slice 的 oomd 清理。后续用户选择仅限制 Rust 内存并取消自动统一排队；避免把日常 75% 建议占用率直接作为杀进程阈值。
