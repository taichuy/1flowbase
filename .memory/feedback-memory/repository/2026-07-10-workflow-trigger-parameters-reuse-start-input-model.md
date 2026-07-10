---
memory_type: feedback
feedback_category: repository
topic: 工作流触发器参数复用 Start 输入字段模型
summary: API/Webhook 触发器的参数定义应复用 Workflow Start 输入字段编辑模型，只额外增加 path/query/body/form 参数来源；不要预生成来源示例行，也不要暴露 target selector 或建立第二套映射配置。
keywords:
  - workflow trigger
  - Workflow Start
  - input_fields
  - path
  - query
  - body
  - form
  - target selector
  - 参数来源
match_when:
  - 设计或修改工作流 API、Webhook、扩展触发器参数配置
  - 调整 Workflow Start 输入参数、请求参数映射或触发器表单
  - 出现 target selector、source/name/target mapping 或默认 path/query/form 参数行
created_at: 2026-07-10 17
updated_at: 2026-07-10 17
last_verified_at: 2026-07-10 17
decision_policy: direct_reference
scope:
  - web/app/src/features/workflow
  - web/app/src/features/agent-flow/components/detail/fields/StartInputFieldsField.tsx
  - web/app/src/features/agent-flow/lib/variables/start-node-variables.ts
  - api/crates/control-plane/src/application_public_api
---

# 工作流触发器参数复用 Start 输入字段模型

## 时间

`2026-07-10 17`

## 规则

API / Webhook 触发器配置参数时，直接参考并复用 Workflow Start 的“新增输入字段”交互与字段模型。每个参数由用户按需新增，核心字段是参数名、参数类型等 Start 输入字段属性；触发器场景只额外增加一个“参数来源”，可选 `path / query / body / form`。

不要再维护 `source + name + target selector` 的第二套映射表，不向用户暴露 `node-workflow-start.xxx` 之类 selector，也不要默认生成 path、query、form 等示例参数行。参数来源是单个输入参数的属性，不是预置参数样例。

## 原因

Workflow Start 本身就是工作流输入参数定义入口。再建立一套触发器参数到 Start 参数的 target selector 映射，会造成重复定义、内部术语泄露、重命名失效和不必要的理解成本。用户截图中的 Start 输入字段编辑器已经提供正确的参数新增模型，只需按触发器协议补充来源属性。

## 适用场景

- API、Webhook、扩展触发器参数表单设计。
- Workflow Start 输入字段 schema、编辑器和后端 DTO 调整。
- 清理 `extension.parameters[].target`、目标 selector 和默认来源参数行。
- 设计触发器请求解析、参数校验和 API 文档生成。

## 备注

用户明确纠正：其“参考 Start 节点参数”的意思不是保留两套定义后自动映射，而是直接复用 Start 参数新增交互和参数模型，并增加参数来源字段。
