---
memory_type: feedback
feedback_category: interaction
topic: 资源占用异常须先剖析取证再解释，不能用常规机理合理化
summary: 用户质疑内存或延迟“不合理”时，先用堆剖析、调用链或采样拿到归因证据，再下结论；不要先用 debug 构建、文件页、分配器保留等通用机理替现状辩护。
keywords:
  - memory
  - profiling
  - jemalloc
  - rationalize
match_when:
  - 用户质疑进程内存、启动峰值、CPU 或延迟偏高
  - 准备用通用机理解释资源占用之前
created_at: 2026-11-27 15
updated_at: 2026-11-27 15
last_verified_at: 2026-11-27 15
decision_policy: direct_reference
scope:
  - api/apps/api-server
  - api/crates/storage/durable/postgres
---

# 资源占用异常须先剖析取证再解释

## 时间

`2026-11-27 15`

## 规则

用户认为资源占用不合理时，先拿证据：jemalloc 堆剖析（`--features tikv-jemallocator/profiling` + 隔离副本 + 克隆库）、累计分配 `prof_accum`、调用链归因与 RSS 时间曲线；不要先用 debug 文件页、分配器保留、启动一次性重活等通用机理替现状辩护。

## 原因

2026-11-27 我先把开发环境 400+ MB 解释为“文件页 + 启动重活，合理”，用户坚持“不合理”。剖析后发现启动期累计分配约 26 GB，其中约 47% 来自本轮提交把全表 `managed_dependency_facts` 加进每次产物写入（O(N²)），属于真实缺陷。

## 适用场景

内存、启动峰值、CPU、延迟等资源质疑；尤其在刚做过优化、用户感知“变化不大”时。

## 备注

剖析时不要用 `prof_gdump`（启动早期写出 2000+ 份快照后死锁）；稳态快照用 gdb 作为父进程调用 `_rjem_mallctl("prof.dump")`；PIE 地址换算用 “运行地址 - 文件偏移 0 映射基址”。
