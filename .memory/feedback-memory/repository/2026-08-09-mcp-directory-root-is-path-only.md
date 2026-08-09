---
memory_type: feedback
feedback_category: repository
topic: MCP 目录树根节点与新建路径草稿必须区分
summary: 用户指出 MCP 目录树根节点只应显示斜杠；新建时只编辑当前路径段，编辑已有分组时只显示路径摘要，不重复显示不可修改的路径输入框。
keywords:
  - MCP management
  - directory root
  - root path
  - create group
  - interaction semantics
match_when:
  - 调整 MCP 实例目录树根节点展示
  - 调整新建分组路径表单或根目录选中态
created_at: 2026-08-09 18
updated_at: 2026-08-09 22
last_verified_at: 2026-08-09 22
decision_policy: direct_reference
scope:
  - web/app/src/features/settings/components/mcp-management
---

# MCP 目录根节点只表达根路径

## 规则

MCP 目录树的虚拟根节点只显示 `/`，实例身份由上方实例选择器表达，不在根节点重复显示。根节点本身不是可编辑分组。新建分组时，父路径由当前选中树节点决定并固定，用户只编辑新分组的单个路径段；前端组合完整 path 后仍以后端 `path` contract 提交。编辑已有分组时，路径不可修改，因此表单不再重复显示只读路径输入框；标题下方的路径摘要是唯一可见路径信息，并且必须对应正在编辑的分组，而不是当前树选择的父目录。

## 原因

实例名称已在选择器中可见，根节点再显示 `instance_name /` 会把实例身份与目录 path 混为一个概念。整条 path 可编辑又会泄漏移动和重命名复杂度；固定父路径、只编辑当前路径段，可以从交互上防止意外改变层级。

## 适用场景

MCP Management 实例目录树、根目录选中态、新建顶层/子分组表单、已有分组编辑表单及可读路径摘要。
