---
created_at: 2026-08-21 21
memory_type: feedback
feedback_category: product_intent
decision_policy: direct_reference
scope: system backup and recovery
---

# 系统备份优先服务迁移与还原

- 规则：讨论或设计开源版系统备份时，先把跨机器迁移、低操作负担和一键还原作为首要目标；不得默认要求用户另行保存部署密钥才能恢复。
- 原因：用户认为备份还原的核心价值是方便迁移；默认把备份绑定到部署 master key 会偏离这一产品原意。
- 适用场景：BackupSet 格式、下载/导入、部署脚本恢复、密钥与加密默认值、灾难恢复和迁移流程。
- 边界：完整性校验仍是必要能力；是否保留可选加密及如何携带已有加密业务秘密，需要在方案确认后实现。
