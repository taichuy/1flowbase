# MCP 按需返回与详情续读

## Tool 默认配置

Tool 的 `basic` 步骤支持两个独立默认值，后端持久化并通过管理 API 与 `mcp_get` 暴露：

| 字段 | 含义 | 未配置时 |
| --- | --- | --- |
| `max_inline_chars` | 本次结果内容的序列化 JSON 字符预算，正整数 | 4,000 字符 |
| `response_fields` | 已映射输出的字段选择，JSON Pointer 字符串数组 | 保留原有映射字段 |

调用者逐项覆盖默认值。预算不叠加旧的 16,000 字符固定上限；不提供无限返回模式。`response_fields: []` 表示不返回业务字段，不能解释为不限范围或不限长度返回。字段选择不是权限配置，不能获取未授权或未映射的业务数据。

管理页面两个默认值均为单行输入框。白名单留空保存为 `null`，输入 `[]` 保存为空数组。输入步骤获取接口参数后会同时展示 `des_id`、`max_inline_chars`、`response_fields` 三个 MCP 调用控制参数；这些参数不进入业务接口的输入映射，调用时位于 `mcp_call` 外层。

## 单次调用

控制参数放在 `mcp_call` 外层，与下游业务 `arguments` 分开，不传入业务接口：

```json
{
  "tool_id": "read_report",
  "arguments": {"report_id": "..."},
  "max_inline_chars": 4000,
  "response_fields": ["/title", "/body"],
  "string_ranges": {
    "/body": {"offset": 0, "length": 1000}
  }
}
```

选择发生在 `output_mapping` 之后。对象使用 `/body`、`/metadata/title`，数组使用 `/items/0/body`；选择父路径会包含其后代。路径中的 `/` 与 `~` 分别转义为 `~1` 与 `~0`，不使用点路径或隐式通配符。

`offset` 从 0 开始，省略时为 0；`offset` 与 `length` 按 Unicode 字符计数，不是字节或 token。`length` 必须为正整数。超出字符串尾部的长度读取到末尾，偏移大于字符串总长度则明确报错。范围路径必须指向已选中的字符串。

## 缓存结果的按需读取

字段选择、字符串范围读取或超长返回会使用原始已映射结果的短期缓存。返回中的 `detail.result_ref` 可用于读取其他字段，不必重新执行业务：

```json
{
  "result_ref": "...",
  "response_fields": ["/body"],
  "string_ranges": {
    "/body": {"offset": 2000, "length": 1000}
  },
  "max_inline_chars": 4000
}
```

`mcp_result` 不传 `cursor` 时开始一个新视图。结果 `entries` 使用字段路径表示；字符串分段附带 `char_offset`、`char_count`、`total_chars`、`next_offset` 和 `complete`。`complete` 表示到达原字符串末尾，而不是仅完成本次范围请求。

若本次选择仍超出预算，传回 `result_ref` 与 `next_cursor` 继续读取。游标绑定字段与字符串范围；后续只传游标即可恢复同一视图。更换选择应省略旧游标，不能把原视图游标用于另一份选择。调整本次字符预算不改变视图。

未覆盖的预算继承该缓存结果对应的 Tool 配置默认值；新视图未指定字段时继承 Tool 默认白名单。缓存快照保留调用时默认配置，不随之后编辑 Tool 而改变。

## 返回预算与失败

预算包含结果内容的 JSON 结构、路径和续读元数据，不只是字符串正文。预算过小、不足以承载一个条目时，返回 `page_budget_too_small`，不静默丢弃正文；这类必要诊断以及 MCP 协议外壳不等同于业务正文预算。

详情缓存沿用现有 10 分钟有效期与存储容量边界；缓存不可用、过期、路径不存在或游标不匹配均明确返回状态。结果读取与业务执行分离，`retry_original: false` 表示不能为了恢复详情重试原操作，尤其不能重复写入。
