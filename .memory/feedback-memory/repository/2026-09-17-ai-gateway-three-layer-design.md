---
title: "AI Gateway 方案保持三层职责并优先成熟机制"
memory_type: feedback
feedback_category: repository
created_at: "2026-09-17 15"
updated_at: "2026-09-26 15"
decision_policy: direct_reference
status: active
tags: [ai-native, architecture, protocol]
---

- 规则：AI Gateway 技术方案明确维持映射协议层 → AI native → 供应商三层；Responses 客户端行为以用户提供的 `/home/taichuy/git/codex` 源码为官方客户端参考，同时核对版本和协议契约。优先评估成熟架构机制、可检验的不变量及合适的数据结构/算法，不堆局部错误分支。
- 原因：用户指出仅围绕一次 409 提出局部修复不足以保证协议、原生语义与供应商边界正确，要求重新给出架构方案。
- 适用场景：AI Gateway 协议映射、工具回传、生成与连接生命周期的诊断和设计。该约束不等于批准任何具体实现或新基础设施。
- 缓存边界：复用 `api/crates/storage/ephemeral`；启动必需的短期高频数据可缓存约 5 分钟，长期低频且已落库的上下文只保存 DB 主键或索引并按需查询。不按猜测的客户端行为或资源环境硬编码业务容量上限。
- 取舍顺序：功能完整性 > 稳定性 > 性能 > 可演进性；诊断质量门禁时不得为收绿牺牲当前协议契约。
