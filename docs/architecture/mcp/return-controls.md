# MCP 按需返回与详情续读

## Tool 默认配置

Tool 的 `basic` 步骤支持两个独立默认值，后端持久化并通过管理 API 与 `mcp_get` 暴露：

| 字段 | 含义 | 未配置时 |
| --- | --- | --- |
| `max_inline_chars` | 本次结果内容的序列化 JSON 字符预算，正整数 | 4,000 字符 |
| `response_fields` | 已映射输出的字段选择，JSON Pointer 字符串数组 | 保留原有映射字段 |

三个调用控制参数与业务参数共用 `input_mapping.interface_parameters` 和 `input_mapping.mappings`。接口参数通过 `source.kind: "mcp_call"` 标识；映射层使用同一套“添加、全部、映射行、删除”操作，只有映射到调用参数的行才开放给模型。未映射时模型不能覆盖默认值，后端拒绝传入的该参数，省略时沿用 Tool 默认值。历史 Tool 没有调用参数标识时沿用原有开放行为；旧版 `call_parameters` 列表仅在读取时保留其选择，编辑后保存为统一映射。预算不叠加旧的 16,000 字符固定上限；不提供无限返回模式。`response_fields: []` 表示不返回业务字段，不能解释为不限范围或不限长度返回。字段选择不是权限配置，不能获取未授权或未映射的业务数据。

管理页面两个默认值均为单行输入框，并校验正整数预算及 JSON Pointer 字符串数组。白名单留空保存为 `null`，输入 `[]` 保存为空数组。输入步骤获取接口参数后，同一接口层字段表展示 `des_id`、`max_inline_chars`、`response_fields` 三个调用控制参数，参数类型均为「JSON 请求体」；映射层与其他参数共用同一张表。未配置必填的描述标识保持可选。调用控制映射行不进入下游业务接口，调用时位于 `mcp_call` 外层。

## 前置说明校验

`des_id` 映射的 `required: true` 要求客户端显式传入当前描述标识；`des_id_required` 由映射派生，不是另一份可编辑真值。该映射不能隐藏或设置非空默认值，运行时也拒绝历史配置用默认值绕过。其他两个返回控制参数仍然可选。

默认校验当前 Tool 自身的标识。若节点工具要求先读取工作流使用说明，可在同一映射的 `source.tool_id` 引用前置工具：

```json
{
  "interface_param": "des_id",
  "mcp_param": "des_id",
  "required": true,
  "source": {
    "kind": "mcp_call",
    "path": "des_id",
    "tool_id": "workflow_ops_create_application"
  }
}
```

此映射与三个 `source.kind: "mcp_call"` 的 `interface_parameters` 描述一起保存。`mcp_get` 暴露 `description_tool_id`，客户端先获取该工具的完整说明，再将其当前 `des_id` 放在节点调用的外层。节点工具自身的标识不能代替前置标识；节点详情不返回前置工具的标识。

引用按当前实例与当前可见 scope 解析；前置工具缺失、停用、解绑或不可见时拒绝调用。只校验引用工具自身的描述，不递归执行或解析其依赖。缺失或过期的标识在业务调用前拒绝。该机制检查客户端持有的说明版本，不证明理解说明或完成前置业务操作；应用、节点、权限与输入仍由后端业务边界校验。

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

缓存分块按实际 JSON 字符串编码字节计费：引号、反斜杠和短控制转义为 2 字节，其余 U+0000–001F 为 6 字节，其他字符为 UTF-8 长度；每块另含两字节引号。splitter 沿字符边界返回原序列化结果的借用切片，只在 CacheStore 接收时拥有该块的 String。所有 chunk 成功后才提交 manifest，缺失或损坏的块不能提供部分完整结果。

inline 判定使用同一 defaults 的 null envelope 长度减 4，再加已序列化 detail 长度；只有确认能完整写入时才构造 owned inline wrapper。delivery 借用原 detail，避免为续读和字段选择提前复制整树。完整结果、既有容量、TTL、cursor、selection、receipt 与权限仍由原 owner 保持；缓存继续拥有正文，不能将局部复制收益外推为进程 RSS 收益。


## 声明式标准内容返回

受管 operation 的 `mcp.output_mapping` 可声明标准 MCP 内容与独立 metadata：

```json
{
  "mcp_content": [{"type": "image", "data_path": "/data", "mime_type_path": "/mime_type"}],
  "structured_content_path": "/metadata"
}
```

路径是显式 JSON Pointer，指向业务响应对象；同一 block 的路径必须有效、互异、互不包含，并且与独立 metadata 对象路径不重叠。宿主只按声明提取，不猜测 stdout。缺失路径、非对象 metadata、非法 mapping 或原生字段明确失败。原有 image 声明与业务响应 wire 保持兼容。

| `type` | 必填路径 | 可选路径 |
| --- | --- | --- |
| `text` | `text_path` | `annotations_path` |
| `image`、`audio` | `data_path`、`mime_type_path` | `annotations_path` |
| `resource_link` | `uri_path`、`name_path` | `mime_type_path`、`description_path`、`annotations_path` |
| `resource` | `uri_path`、`text_path` / `blob_path` 恰好一个 | `mime_type_path`、`annotations_path` |

图片和音频必须是非空、规范 base64，MIME 分别满足合法 `image/*`、`audio/*`，不限定业务 MIME 白名单；空 resource blob 是合法空文件。资源 URI 必须绝对，宿主不下载或解析目标。嵌入资源输出 `{type:"resource",resource:{uri,mimeType?,text|blob}}`，合法 annotations 保留。`resource_read` 默认 false，启用要求至少一个嵌入资源，可同时包含 resource_link；含 text、image 或 audio 的 mapping 不允许启用，声明可接入资源读取 adapter。

本地声明的全部原生内容（含 text）独立于 metadata 的选择、文本分页与续读缓存。`max_inline_chars`、字段/范围选择和详情续读只作用于独立 metadata 对象，低 metadata 预算不截断原生 block。上游 proxy 的 text 继续保持现有 metadata 预算语义，非文本内容在分页时保留。

最终 JSON-RPC 256 KiB envelope 只豁免通过验证的 `image.data`、`audio.data`、嵌入 `resource.resource.blob` 和 `resources/read` 的 `contents[].blob`。base64 验证复用固定大小 decode buffer，不额外分配完整 blob。MIME、URI、annotations、资源 text、metadata、未知字段与 JSON 结构仍计入预算；非法 binary 字段不豁免。

业务 `output_schema` 保留原响应契约。`mcp_get.result_schema` 使用声明 metadata 路径对应的真实业务子 schema；无法静态解析时返回未约束 schema，不伪造字段。省略 `mcp.output_mapping` 保持原有纯文本投影。

## 资源重读

服务器在 tools 之外声明 `capabilities.resources = {}`，保留原协议版本 `2025-03-26`。`resources/list` 返回 `{resources:[]}`；工具产出的资源不承诺进入资源目录。

InterfaceWrapper 的 mapping 显式设置 `resource_read:true` 时，嵌入资源与指向相同原 URI 的 resource_link 使用宿主 URI：`flowbase-resource://read/<BASE64URL_JSON>`。一般外部链接保留原 URI，不自动 fetch。自包含、带版本的引用只携带用户/workspace、所选 instance、tool ID、当前 `des_id`、调用者原业务 arguments 与原 resource URI，不含响应 binary、请求 headers、session 凭证或 hidden/default bindings；不维护 URI registry 或 binary cache。该声明用于输入参数可安全作为 locator 暴露的文件读取操作。

`resources/read` 接收 `{uri}`，返回 `{contents:[{uri,mimeType?,text|blob}]}`。返回 URI 保持请求的宿主 URI，不递归包装。引用只定位资源，不授予权限：每次读取在调用前检查当前用户/workspace、所选且可见的工具/instance、显式 opt-in 与当前 revision。可见性撤销、opt-in 关闭、revision 改变、非法或未知宿主 URI 均在工具调用前拒绝。修改 locator 的 arguments 不扩大访问权限。

RPC 经现有 Invocation Kernel 与 ToolCall port，重新投影当前受管工具，经同一 InterfaceDispatch 与宿主 executable 检查执行原读取工具。只返回成功结果中与原 URI 相同的嵌入资源；失败不返回部分结果。重读会再次执行原业务读取，可能观察到文件变化。编码与解码均服从现有 2 MiB tool arguments 真实传输预算，payload 字段与版本严格校验。
