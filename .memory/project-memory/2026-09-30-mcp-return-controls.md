---
memory_type: project
topic: mcp-return-controls
summary: 用户批准 Issue #2176 的按需返回方案；本地实施并完成定向验证，完整宿主 HTTP 运行测试仍待验证。
keywords: [mcp, return-controls, issue-2176]
match_when: [继续 MCP 返回控制实施或验收]
created_at: 2026-09-30 23
updated_at: 2026-09-30 23
last_verified_at: 2026-09-30 23
decision_policy: verify_before_decision
scope: [MCP Tool 配置与缓存续读]
---

# 已批准的按需返回

- 谁在做什么：用户于 2026-09-30 明确批准创建单一 Issue #2176 后实施；Codex 完成本地前后端实现。
- 为什么这样做：默认预算与白名单可被调用逐项覆盖，字符串范围与字段选择复用缓存结果，不重执行业务。
- 为什么要做：减少上下文消耗，同时避免固定分页妨碍模型按需读取。
- 截止日期：未设置；Issue 在用户验收前保持打开。
- 决策动机：保留有界返回，不提供无限返回，不叠加原 16000 固定字符硬上限。
- 验证边界：定向领域/契约、前端、真实缓存与隔离 PostgreSQL 测试已通过；完整 API 宿主测试因物理内存边界未运行完成，不据此宣称部署验收完成。
- 当前真值：Issue #2176 与 docs/architecture/mcp/return-controls.md；继续工作先核对代码及最新 QA 证据。
