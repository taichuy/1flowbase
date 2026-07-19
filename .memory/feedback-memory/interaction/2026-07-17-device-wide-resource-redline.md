---
memory_type: feedback
feedback_category: interaction
topic: device-wide-resource-redline
summary: 用户规定的 CPU、物理内存、存储 90% 红线按整台设备聚合利用率判断；raw swap 占用只作观察，不单独阻断。
keywords:
  - resource budget
  - CPU
  - memory
  - physical memory
  - storage
  - swap
  - Cargo
  - multi-agent
created_at: 2026-07-17 00
updated_at: 2026-07-19 13
last_verified_at: 2026-07-19 13
decision_policy: direct_reference
scope:
  - multi-agent development
  - Cargo validation
---

# Device-wide resource redline

## Rule

多 agent 或多 worktree 工作时，CPU、物理内存与存储红线按整台设备的聚合指标计算，三者均须≤90%。启动新的 Cargo、服务或重型验证前，检查设备总 CPU 利用率、物理内存压力与存储使用率；单一进程的 `100%` 只代表一个逻辑核，不是红线判断依据。

raw swap 已用比例不是独立硬停止条件：Linux 可以保留已换出的冷页，即使物理内存仍充足。只有 CPU、物理内存或存储达到红线时停止；swap 可作为诊断记录，结合持续 swap-in/out、OOM 或物理内存压力解释风险，但不能单独把 QA / 构建判为环境阻塞。

## Reason

用户的资源约束旨在保护整个开发设备和并行工作，不是限制某一条命令的单核占用。错误地按单进程百分比判断会误判可用容量或不必要地阻塞验证。

## Applies When

- 计划启动 Cargo、Rust 编译、服务或其他重型进程。
- 检测到其他 worktree、用户进程或 agent 已在运行构建。
- 需要决定是否允许并行或串行验证。

## Practice

用设备级 CPU 采样、物理内存可用量和存储使用率判断是否低于 90%；仍要避免同一 worktree 的 Cargo 并发和不必要的构建缓存争用。达到或接近红线时暂停新进程，已运行的用户/外部进程不得擅自终止；只因 swap 高而物理资源安全时，继续单一重型进程并记录观察值。
