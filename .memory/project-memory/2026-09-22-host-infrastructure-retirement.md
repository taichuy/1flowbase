---
title: 独立基础设施设置退役
summary: 用户批准删除旧配置入口及专属链路，保留扩展中心、基础设施运行实现和内存观测。
status: active
decision_policy: verify_before_decision
created_at: 2026-09-22 07
updated_at: 2026-09-22 07
---

用户于 2026-09-22 明确批准平衡方案，并要求在独立 worktree 实施后合并回来。动机是撤掉早期冗余后台入口，不再把旧 Provider 配置表单搬进扩展中心，也不借此补建新基础设施配置体系。

边界：保留宿主扩展安装、启停、版本管理与实际运行实现；内存观测保留，缓存操作归入其权限分组。移除旧 Provider 配置链路与卸载阻挡，旧配置和原始角色授权归档，合并权限不得扩权。

Root 已于 2026-09-22 完成实现并将 cb7533991 合入、推送 dev；任务实施截止日期为当日，无继续开发安排。正式迁移随新版本后端启动执行，本轮只做数据库回滚预演。验证记录在 tmp/test-governance/host-infrastructure-retirement/receipt.md。后续若要新增宿主扩展配置，需另行对齐产品需求，不恢复旧页面作为默认方案。
