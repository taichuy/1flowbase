---
title: "工作流试运行受理即进入日志列表"
memory_type: project
created_at: "2026-11-15 18"
updated_at: "2026-11-15 18"
decision_policy: verify_before_decision
status: active
tags: [workflow, runtime, application-logs]
---

- 用户于 2026-11-15 明确要求在本地 dev 复现并修复试运行日志缺失，允许从 `1flowbase_gateway` 开发库抽取部分数据。授权仅覆盖开发库样本复制与 dev 修复；7600 保持只读观察，不重启、不停止，不修改 gateway 源库。
- 目标与动机：纯代码/HTTP 工作流离开编排页后仍可在日志列表发现同一次运行，不依赖 LLM 节点或终态才生成摘要。任务受理、排队到运行与列表投影共享事务，投影失败时整体回滚，防止不可见的已受理任务。
- 实施范围：创建 flow run shell 和绑定 compiled plan 的既有 repository 写入口；不更改前端、不添加 migration、不为 GET 添加修复写入，不扩展清理 worker 性能优化。
- 样本：从 gateway 只读抽取 7 条关联记录，52,787 字节；私有样本权限 0600，保留在 `tmp/test-governance/workflow-run-visibility/gateway-sample.private.json`，不提交。隔离测试只恢复文档/输入并调用持久化入口，不执行原工作流的 HTTP/删除节点。
- 验证：旧实现复现受理后列表 0 条；修复后的 24 项 PostgreSQL 生命周期/任务列表回归通过，默认合成夹具另有 1 项通过。覆盖 GUI 的 runtime model reader、queued/running/succeeded、创建与状态更新的故障回滚。正式 migrations 运行于自动回收的一次性数据库；测试遵循本地单 job 与 Rust 预算。
- 证据：`tmp/test-governance/workflow-run-visibility/`。仅确认 dev 源码与定向数据库行为；未部署或重启 7600，未回填 gateway 历史缺失记录，未执行全仓后端门禁。现有 `inject_workflow_credentials` unused import warning 不属于本轮修改。
- 集成：经闲置且干净的隔离 worktree 复用为 `codex/workflow-run-log-visibility`，与已验证源文件逐字节核对后，由 Root 集成到 dev。提交与推送结果以 git 为准。无新增截止日期；后续部署或历史数据修复需依据当时授权。
