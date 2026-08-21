---
memory_type: project
topic: Frontstage code block uses a persisted short title
summary: 用户确认新增代码区块直接显示并持久化 8 位稳定短标识；设置面板标题必须编辑同一节点 title，不再写入 runtime props.title。
keywords:
  - frontstage
  - block title
  - short identifier
  - TSX editor
  - block_id
match_when:
  - 修改 Frontstage 区块创建默认标题
  - 修改 TSX 编辑器设置面板标题
  - 修改区块树、画布或编辑器的区块名称显示
created_at: 2026-08-21 16
updated_at: 2026-08-21 16
last_verified_at: 2026-08-21 16
decision_policy: verify_before_decision
status: active
scope:
  - api/apps/api-server/src/routes/frontstage/block_tree.rs
  - api/crates/control-plane/src/frontstage/block_tree.rs
  - web/app/src/features/frontstage
---

# Frontstage 区块短标题

- 谁在做什么：后端在创建区块时拥有最终 `block_id`，从其稳定派生并持久化 8 位标题；前端区块树、画布设计标签和 TSX 编辑器设置面板消费或更新该节点标题。
- 为什么这样做：默认“代码区块”会让多个独立实例无法辨认；另存随机标题会造成状态重复，前端临时 UUID 也不等于后端最终 UUID。
- 为什么要做：用户要求标题仅显示短标识（如 `K7M2PX9Q`），并期望编辑器的“标题”输入能真实修改看到的区块名称。
- 截止日期：未指定；现有标题为“代码区块”的历史记录不批量改写，仍可由用户在设置面板手工更新。
