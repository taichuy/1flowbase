---
title: "DataTable 新增列必须迁移持久化字段配置"
memory_type: feedback
feedback_category: repository
created_at: "2026-07-31 16"
updated_at: "2026-07-31 16"
decision_policy: direct_reference
status: verified
tags: [frontend, data-table, persistence, migration]
---

# Rule

给支持字段配置持久化的 DataTable 新增默认可见列时，必须验证已有 `localStorage` 或用户偏好状态；新 schema 列应自动加入可见列，同时保留用户已主动隐藏的旧列。

# Reason

只修改 columns schema 会让旧 `visibleColumnKeys` 继续隐藏新列，开发时清空状态或新浏览器截图会误判功能已经可见，用户重启后仍看不到新增能力。

# Applies To

所有使用 `usePersistedDataTableConfiguration` 或 `useUserPreferenceDataTableConfiguration` 的表格列新增、重命名和删除；验收必须包含旧状态 fixture 与真实浏览器 reload 证据。
