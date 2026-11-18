# 编排配置诊断

[English](validation-diagnostics.en.md)

`orchestration-runtime` 拥有节点配置、绑定及拓扑校验。可收集的错误继续保存在 `CompileIssue`，其中 `field_path` 是可选的定位信息；无法生成有效计划的配置错误通过 typed `FlowValidationError` 携带 `NodeDiagnostic`。纯编译入口将配置错误归类为 validation，不把数据库、网络或供应商执行异常统一归类为参数错误。

服务层保留诊断。单点预览仍按目标及上游依赖范围检查可收集的 compile issues；这不意味着自动执行上游，也不改变原有整图结构校验。拒绝发生在创建运行记录及执行节点之前。

公共 API 返回 HTTP 400，顶层 `code=flow_validation_failed`，诊断位于 `details`：

```json
{
  "phase": "validation",
  "diagnostics": [{
    "node_id": "node-aggregate",
    "code": "variable_aggregator_output_mismatch",
    "field_path": "/outputs/0/title",
    "message": "variable_aggregator output title must match group result",
    "expected": "result"
  }]
}
```

`node_id` 存在时，`field_path` 是相对于该节点的 JSON Pointer，键名中的 `~`、`/` 使用标准转义。没有节点 ID 的诊断定位整份文档。空指针表示整个节点或文档，不能伪造无法准确确定的字段位置。`expected` 只在校验规则拥有具体约束时提供。节点 ID、规则 code、字段路径由校验 owner 产生，不从异常字符串反向解析；公开 message 经现有 API 脱敏规则处理。

MCP 复用现有公开 API `details` 投影，返回 `target_code` 和 `target_details`；认证、授权及内部故障详情不走配置诊断投影。API 的 code、details 为真值，GUI 与 MCP 不维护另一套节点校验器。

编译前失败不会伪造 `node_run`。节点已执行后发生的失败继续使用持久化 `node_run.error_payload` 及已有状态/终态逻辑；真实内部故障仍为 HTTP 500。

## 草稿分析与输出契约

`POST /api/console/applications/{id}/orchestration/analysis` 接收 `{ "document": ... }`。它使用所属应用的类型、权限与编译上下文分析未保存文档，不创建编辑状态、不保存版本、不执行节点。其 Canonical Interface 为 `applications.orchestration.analysis`，GUI 与 MCP wrapper 使用同一入口。

返回 `diagnostics`、`node_outputs` 和 `variable_catalogs`。诊断与输出契约覆盖整份文档；请求可选 `node_id` 时仅生成该目标节点的变量目录，未选择节点时目录为空，避免大流程展开全图传递闭包。诊断的 `level` 为 `error` 或 `warning`；输出契约里的 selector 是节点结果内的相对路径，变量目录里的 selector 包含源节点 ID。目录按目标 `node_id` 和节点内 JSON Pointer `field_path` 分组，空路径提供默认目录，具体 binding 路径表达容器等位置的作用域。目录只包含声明的公开输出及作用域内变量，不暴露环境变量值。

Code 执行结果保持 `{ "result": main返回对象, "error": null }`。显式 `outputs[].selector` 是真值，包括指向整个对象的 `["result"]`；未填写 selector 时才默认采用 `["result", key]`。例如 `main` 返回 `{body:"...",count:7}`：

```json
{
  "key": "result", "title": "打包结果", "valueType": "object",
  "selector": ["result"],
  "jsonSchema": {
    "type": "object", "additionalProperties": false,
    "properties": {"body":{"type":"string"},"count":{"type":"integer"}},
    "required": ["body","count"]
  }
}
```

下游可以引用 `["pack","result"]` 或 `["pack","result","body"]`。若代码本身返回名为 `result` 的成员，则用 `["result","result"]` 明确表示该嵌套路径；不能通过猜测历史 `key` 的意图自动迁移。

后端共用输出路径及词法作用域检查。封闭 Schema 的未知属性、标量后取子属性会报错；允许额外属性的动态 JSON 路径保留为 warning，运行时仍须存在。`$ref` 的本地引用、组合 Schema 与同层属性约束共同参与判断。前端消费后端目录，不另外补齐 Code selector 或虚构未声明的 HTTP 输出。

保存、导入和恢复先拒绝非法基础结构、输出 selector 或 Schema；语义尚未完成的草稿仍可保存，编辑状态响应携带 `analysis`。发布及启用定时任务前拒绝 error，warning 不阻断。定时启用同时检查当前草稿、已发布快照和实际执行计划。发布冻结后再次检查真实快照，避免激活预检后被并发改坏的定义；现有冻结读写间隙不提供事务级 CAS 保证。

Code 缺少 source/source_ref 会产生 `/config/source` 的就绪性错误。执行后，显式声明的输出路径缺失会在 Code 节点失败；整对象的可选成员与可空成员由现有 JSON Schema 的 `required` 和类型表达。静态分析不执行 JavaScript，也不证明其任意业务分支都会返回正确值，执行时仍验证真实输出。
