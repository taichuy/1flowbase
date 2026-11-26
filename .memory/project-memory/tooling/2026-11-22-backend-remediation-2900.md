---
memory_type: project
topic: 后端架构、质量门禁与第三方依赖系统性收敛（Root #2900）
summary: 用户批准平衡方案两层 Issue Tree（#2900 -> #2901/#2902/#2903）及轻量 S3 SigV4 重写 rustfs，全部 9 项 AC 通过独立集中 QA（Cycle 2 QA_PASS）并经用户确认快进合入 dev、关闭 #2900-#2903。
keywords:
  - backend-remediation
  - issue-2900
  - rustfs
  - s3-sigv4
  - postgres-test-support
  - dependency-boundaries
  - ephemeral-cache
match_when:
  - 处理 1flowbase 后端测试数据库模板克隆、门禁 CRLF/缓存治理、storage-object rustfs S3 驱动、Cargo.lock 依赖基线或协议层禁 SQL 边界时
created_at: 2026-11-22 15
updated_at: 2026-11-22 16
last_verified_at: 2026-11-22 16
decision_policy: verify_before_decision
scope:
  - api/
  - scripts/node/
---

# 后端架构、质量门禁与第三方依赖系统性收敛（Root `#2900`）

## 标题

Root `#2900`（`D1 #2901` + `D2 #2902` + `D3 #2903`）通过独立集中 QA 并快进合入 `dev` 收口关闭

## 时间

`2026-11-22 16`

## 谁在做什么

开发者与 AI 完成 `1flowbase` 后端两层 Issue Tree（Root `#2900` -> `D1 #2901` 测试资源治理与门禁去脆弱化、`D2 #2902` 原生轻量 S3 SigV4 重写 `rustfs` 与依赖瘦身、`D3 #2903` `ephemeral` 缓存纠偏、业务容量解绑与全部 6 个 `api-*` 协议 crate 裸 SQL 下沉），经第二轮独立集中 QA（`1,025` 项测试全绿，测试库 `0` 残留）及用户验收确认，快进合入 `dev` 并关闭 `#2900`–`#2903`。

## 为什么这样做

采用现有 `reqwest`（`ring` TLS）+ `hmac` + `sha2` 重写轻量 S3 SigV4、PostgreSQL `IS_TEMPLATE` 种子模板克隆与标准库 `BinaryHeap`/`OnceLock`，在零新增第三方依赖、零破坏现有接口契约且不触碰 `7600` 观察样本的前提下消除历史技术债。

## 为什么要做

解决前期快速迭代遗留的测试库泄漏（97 个残留库）与全量建表高开销、Windows CRLF 下静态门禁哈希失配、单驱动拉入整套 AWS SDK/双 TLS 原生构建链、`application_log_cache` 无失效脏读、硬编码业务容量上限及协议层散落裸 `sqlx` 查询问题。

## 截止日期

已于 `2026-11-22 16` 合入 `dev` 完成交付收口；`rustfs.rs` 中 `path_style` 兼容别名按 `@field-contract-compat` 计划于 `2027-06-01` 移除。

## 决策背后动机

遵循优化决策优先级（功能完整性 > 稳定性 > 性能 > 可演进性）、不引入未授权第三方依赖及奥卡姆剃刀原则。

## 关联文档

- GitHub Issues：`#2900`（Root）、`#2901`（D1）、`#2902`（D2）、`#2903`（D3）
