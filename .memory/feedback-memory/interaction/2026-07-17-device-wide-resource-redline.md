---
memory_type: feedback
feedback_category: interaction
topic: device-wide-resource-redline
summary: 用户规定的 CPU/内存 90% 红线按整台设备聚合利用率判断，不按单个 Rust/Cargo 进程或本任务占用判断。
keywords:
  - resource budget
  - CPU
  - memory
  - Cargo
  - multi-agent
created_at: 2026-07-17 00
updated_at: 2026-07-17 00
last_verified_at: 2026-07-17 00
decision_policy: direct_reference
scope:
  - multi-agent development
  - Cargo validation
---

# Device-wide resource redline

## Rule

多 agent 或多 worktree 工作时，CPU 与内存红线按整台设备的聚合指标计算。启动新的 Cargo、服务或重型验证前，检查设备总 CPU 利用率与总内存压力；单一进程的 `100%` 只代表一个逻辑核，不是红线判断依据。

## Reason

用户的资源约束旨在保护整个开发设备和并行工作，不是限制某一条命令的单核占用。错误地按单进程百分比判断会误判可用容量或不必要地阻塞验证。

## Applies When

- 计划启动 Cargo、Rust 编译、服务或其他重型进程。
- 检测到其他 worktree、用户进程或 agent 已在运行构建。
- 需要决定是否允许并行或串行验证。

## Practice

用设备级 CPU 采样和总内存可用量判断是否低于 90%；仍要避免同一 worktree 的 Cargo 并发和不必要的构建缓存争用。达到或接近红线时暂停新进程，已运行的用户/外部进程不得擅自终止。
