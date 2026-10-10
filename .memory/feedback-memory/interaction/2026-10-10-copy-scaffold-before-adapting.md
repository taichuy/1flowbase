---
memory_type: feedback
feedback_category: interaction
topic: repository-scaffold-copy-first
summary: 复制已有仓库结构且排除业务内容的明确任务，先复制基础设施再清理调整与验证提交，避免过长的前置分析。
keywords: [repository, scaffold, copy, execution]
match_when: [用户要求从已有仓库复用结构和打包流程建立新仓库]
created_at: 2026-10-10 17
updated_at: 2026-10-10 17
decision_policy: direct_reference
---

# Copy Scaffolds Before Adapting

规则：源和目标及排除范围已明确时，完成必要的本地规则、Git 状态检查后，直接复制允许的文件，再清理业务内容、调整仓库身份、验证并在授权范围提交推送。

原因：用户明确纠正了本次商业仓库初始化中过长的前置检查，希望通过实际文件变更推进工作。

适用场景：已有成熟仓库的结构/构建基础设施复制；不据此跳过目标已有内容保护、私有性核验或产品语义未明确时的必要对齐。
