---
created_at: 2026-06-11 13
updated_at: 2026-08-05 10
memory_type: project
decision_policy: verify_before_decision
scope: qa gate lane model
---

# QA Gate Lane Model

用户在 2026-06-11 确认：质量门禁按三个场景分 lane，而不是把所有 QA 场景混成同一套重门禁。

已确认 lane：

- `Dev Acceptance Gate`：开发后功能验收，目标是快；复用 TDD 红绿结果，按风险向量选择最小证据链，证据足够或预算耗尽就停。
- `PR Merge Gate`：PR 合并门禁，目标是合并信心；优先 GitHub Actions / artifact，报告 blocker、warning、advisory、资源耗时和合并风险。
- `Project Health Gate`：项目体检全量门禁，目标是维护者感知和项目治理；优先远端完整门禁与 artifact，本地 AI 读取证据、输出健康快照、风险热力图、趋势、轮转深挖和维护建议。

算法化方向：

- 开发后验收使用风险向量、最小证据链、时间预算和早停。
- PR 门禁使用 gate DAG、并行调度、失败归因、合并风险评分和成本报告。
- 项目体检使用质量维度矩阵、风险热力图、趋势对比、依赖中心性、分层抽样和轮转深挖。

项目体检发现硬性门禁失败时，可以进入质量回归修复；非硬性维护问题应联动 `problem-framing`，按现状、方向、风险收益和建议给维护者决策。

## 2026-08-05 四大基座契约证据治理

用户批准在三条既有 lane 之上增加 `AI Gateway / MCP Gateway / Application Backend / Native React frontend blocks` 四大基座契约轴，并创建 Single Issue [#1597](https://github.com/taichuy/1flowbase/issues/1597) 作为实施与验收真值。

硬边界：质量门禁只提供 CI / QA 证据与管理员判断依据；不启用 required check，不修改 branch protection / repository ruleset，不限制管理员手动合并，不改变产品 API、数据、状态、权限或用户可见 UI。MCP 产品主链仍为 `mcp.list -> mcp.get -> mcp.call`，`mcp.result` 仅在大结果或 durable receipt 场景作为内部续取证据。

用户在 2026-08-05 追加执行约束：集中完成改动后只做一次集中本地回归，再推送 GitHub Actions；单个门禁最长执行路径低于 1 小时。若少于 3 个基座门禁反复失败，先本地单基座复跑，再线上单基座复跑，最后恢复 `auto/all`。功能完整性和用户可见 UI 是硬边界，已证实无效的旧门禁应及时删除，而不是为旧断言保留兼容。
