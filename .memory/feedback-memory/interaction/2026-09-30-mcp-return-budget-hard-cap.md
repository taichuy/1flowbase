---
memory_type: feedback
feedback_category: interaction
topic: mcp-return-budget-hard-cap
summary: 用户已确认 MCP 返回预算不叠加 16000 固定硬上限、不提供全部返回选项，并组合字段选择、字符串范围读取与 result 续读。
keywords: [mcp, max_inline_chars, 返回长度, 硬上限]
match_when:
  - 讨论 MCP Tool 默认返回长度与调用参数覆盖
created_at: 2026-09-30 21
updated_at: 2026-09-30 23
last_verified_at: 2026-09-30 23
decision_policy: direct_reference
scope: [MCP 返回策略]
---

# 调用预算不等于固定硬上限

## 规则
不把额外固定字符上限自动加入已确认方案。明确默认值、调用者覆盖值、实际传输失败的区别；用户已确认调用参数逐项覆盖 Tool 默认预算与字段选择，不叠加原 16000 固定字符上限。

2026-09-30 用户明确不增加“全部返回 / 不按长度拆分”选项，防止长任务撑爆上下文；遇到返回预算限制使用 result 续读。用户确认组合返回白名单与指定字符串 offset/length，避免只能顺序翻页，并授权创建 Issue #2176 后实施。字段选择不是权限；已有缓存容量与有效期是存储边界，不是新的返回字符硬上限。

## 原因
额外硬上限会使超出该值的显式调用预算失效，削弱用户要求的自主返回控制。

## 适用场景
MCP Tool 配置、单次返回预算、字段选择和详情续读方案讨论。
