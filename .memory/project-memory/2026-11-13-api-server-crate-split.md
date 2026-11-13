---
memory_type: project
topic: api-server-crate-split
summary: api-server 编译内存治理：用户确认平衡方案，按域拆为 crates/api/ 下 7 个 crate（MCP 独立为 api-mcp）；计划草稿已写，Root Issue #2824 已建，未改产品代码。
created_at: 2026-11-13 12
updated_at: 2026-11-13 12
decision_policy: verify_before_decision
scope: [backend, api-server, build-memory]
---

## Approved scope
用户于 2026-11-13 确认：走平衡方案；新 crate 放 `api/crates/api/`；MCP 独立成 `api-mcp`；整理为文档计划。硬原则：不能破坏功能完整性。

## Why
`api-server` 单个 rustc 进程冷编译峰值 9.1–9.5 GiB（基线 `beta@5104e422d`），测试编译有 5 个约 9 GiB 的单元；用户希望开发与测试编译落在 8 GiB 内。主因是单 crate 25 万行（路由 19.4 万行）的单态化，不是模块数量。

## Active control
- 唯一计划真值：taichuy/1flowbase Root Issue #2824（Delivery：D1 #2825、D2 #2826、D3 #2827、D4 #2828、D5 #2829），phase:ready。尚未 Scout、未改产品代码；用户计划在新会话启动。
- 本地草稿副本：`docs/draft/api-server-crate-split-plan-20261113.md`；测量证据与脚本：`tmp/test-governance/compile-memory-audit/`（本地，可能被清理）。
- 无承诺交付日期。

## Confirmed decisions（用户，2026-11-13）
保留三个 `include!` 测试根与 `api-test-selection`；第七个 crate 为 `api-console-interaction`；阈值：单 rustc 进程 ≤4.0、dev 构建 ≤7.5、测试编译 ≤8.0、链接 ≤3.9 GiB（cargoJobs=2）；单一 Root，每个 Delivery 边界变基到 beta。

## Notes
降幅数字是模型预测，未做原型验证；D1 装配后复测是校准点（下降不足 1.0 GiB 则停止 reframe）。
