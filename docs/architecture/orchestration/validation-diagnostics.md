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

编译前失败不会伪造 `node_run`。节点已执行后发生的失败继续使用持久化 `node_run.error_payload` 及已有状态/终态逻辑；真实内部故障仍为 HTTP 500。此变更不改变节点配置规则、业务写入政策、缓存或供应商协议。
