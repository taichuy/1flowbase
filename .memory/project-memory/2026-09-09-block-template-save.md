---
title: 区块编辑器保存页面模板草稿
date: 2026-09-09 16
decision_policy: verify_before_decision
---
用户确认由 Codex 在区块编辑器代码模板面板新增“保存为页面模板”，单任务实施。保存当前代码（含未保存修改），弹窗填写名称，复用 UI 管理模板创建权限及 API，保存为未发布草稿。目的是直接复用当前区块，减少复制粘贴；不保存整页布局和业务数据，不自动发布。无截止日期。
验收：当前草稿与模板源码一致，模板管理可见；名称校验、失败保留输入和无权限禁用；桌面与手机弹窗可操作。真实开发环境创建与读取验证通过，QA 模板已归档；临时认证 session 已回收。证据在 tmp/test-governance/block-template/ 与 block-template-page/。全局类型检查有未修改 billing-panels.test.tsx:195 的 TS7006，任务文件无类型错误。

2026-09-09 17 用户明确要求将代码模板归档改为前后端永久删除；随后明确已有归档模板恢复普通列表、保留原内容。已实现 DELETE 模板、修订及默认关联的原子级联删除，移除归档字段/入口/旧接口。开发环境迁移和重启已完成；旧归档名称和源码保持不变；临时模板真实删除后列表/发布列表均不再出现。证据 tmp/test-governance/template-delete/acceptance.md。原 archive 自定义授权不升级为破坏性 delete，custom 角色需独立授权。
