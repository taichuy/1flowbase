---
memory_type: project
topic: AI Gateway V1 单一 contract Delivery Tree
summary: 旧 #1303 树已关闭；唯一活动 Root 为 #1366，包含 5 个纵向 Delivery 与 14 个初始 Work Packet，当前仅完成建树，尚未授权产品开发。
keywords:
  - issue 1366
  - AI Gateway V1
  - single contract
  - Issue Tree Root
  - Delivery Tree
created_at: 2026-07-17 22
updated_at: 2026-07-18 18
last_verified_at: 2026-07-18 18
decision_policy: verify_before_decision
scope:
  - https://github.com/taichuy/1flowbase/issues/1366
  - /home/taichuy/git/1flowbase
  - /home/taichuy/git/1flowbase-official-plugins
---

# AI Gateway V1 Single Contract

## 当前活动真值

- 唯一活动 Root 是 #1366；#1303 及其旧 Delivery 已关闭归档，不作为后续 agent 默认上下文。
- Root 当前为 `phase:discussion`，用户只授权完成 Scout、Delivery Tree 与 Work Packet 梳理，尚未授权产品开发或测试。
- 主仓 protected baseline：`dev@9a0960fd30a20cf0a604e1d09b07ac35e8500b95`。
- 官方插件 baseline：`main@4c6f70be5284d16900924a9e2a7f677cd88c80c7`。

## Delivery Tree

1. #1367 Terminal winner，`phase:ready`，Packet T1～T2。
2. #1371 Current Generate contract，`phase:ready`，Packet G1～G4。
3. #1368 Publication Operation Binding，`phase:discussion`，Packet B1～B3。
4. #1369 Canonical CountTokens，`phase:discussion`，Packet C1～C2。
5. #1370 Compact candidate / CAS，`phase:discussion`，Packet K1～K3。

依赖为 `#1371 → #1368 → #1369`，且 `#1368 → #1370`；#1367 可独立形成 assembly 输入。Conformance、rollback 与 QA 属于 Root 集中 Test Batch，不创建 QA Delivery。

## 未决门与执行边界

- DG-1 / #1368：binding identity、旧 publication backfill、Console 编辑权限。
- DG-2 / #1369：六个官方 Provider/profile 的 CountTokens capability matrix。
- DG-3 / #1370：Compact artifact 内容、保留期限、删除语义与用户可见性。
- 未冻结的 Delivery 不得实现；全部开发与 fixture 装配完成后，才允许一个 fresh Root QA。
- Root agent 是唯一调度、assembly 与 Control Ledger owner；Delivery agent 只接收当前 Work Packet handoff，不继承旧树或完整历史。

无固定时间预算；仅记录 Packet、needs-split、assembly conflict、agent context、validation run 与 QA cycle 等可验证事件计数。
