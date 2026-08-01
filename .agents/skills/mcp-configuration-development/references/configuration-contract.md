# Configuration Contract

## Ownership Table

| 配置对象或字段 | 负责表达 | 不负责表达 |
| --- | --- | --- |
| Instance | 一套 Virtual UI 的身份、默认入口和启用状态 | 业务权限或具体动作 |
| Group `path` | canonical 导航层级 | HTTP path、组件树 |
| Group `display_name` / `description_short` | 领域入口名称与下钻方向 | Tool 的完整调用说明 |
| Tool `tool_id` / `name` | 稳定能力身份与用户可读动作名 | 临时任务编号 |
| `short_description` | 调用的直接作用与可观察结果 | 参数逐项解释、内部实现 |
| `full_description` | 不可拆分的组合契约与复杂上下文 | 短描述改写、路径复述、普通字段说明 |
| `parameter_schema` / `result_schema` | Agent 输入与结果的结构 contract | GUI 展示布局 |
| `input_mapping` | Agent 参数到接口参数的命名、说明、必填性与映射 | 业务接口本身不存在的语义 |
| `output_mapping` | 接口结果到 Agent 结果的必要投影 | 复制 result Schema、隐藏真实错误或伪造成功 |
| `permission_code` / `risk_level` | 权限标识与调用风险 | 导航分组 |
| Binding | Tool 在实例中的挂载、可见性、别名和顺序 | Tool 定义副本 |
| discovery policy | 列表深度、数量、搜索和返回字段 | 业务授权规则 |
| `children_count` | 运行时导航提示 | 可编辑或持久化配置 |

## Description Rules

### Short Description

用一句话回答“调用后会发生什么或得到什么”。优先写可观察结果，例如“创建工作流草稿并返回其标识”，不要写“用于工作流相关操作”。

### Full Description

默认值是空字符串 `""`。仅当以下条件同时成立时填写：

1. 存在跨字段、跨 Tool、跨状态或跨制品的组合关系；
2. 其他专用字段无法独立表达；
3. 缺少说明会让 Agent 以错误顺序、错误引用或错误数据形状调用。

适合填写：创建复合工作流时节点与连接器的引用规则；创建 Tool 挂载时多模态拦截配置与相关制品的组合约束。

不适合填写：short description 的扩写、HTTP path、权限文案、字段逐项解释、源码实现细节、临时选择理由。

## Input Mapping

以当前配置模型为准：

- `interface_parameters[]` 保存接口参数的原始名称、类型、位置、说明和必填性。
- `mappings[]` 保存 `interface_param → mcp_param` 的 Agent-facing 重命名、说明和必填性。
- 存在 mapping 时，Agent 看到的 input Schema 应使用 `mcp_param`、mapping description 与 mapping required。
- 没有重命名时可沿用接口参数，但仍应检查原始 description 是否足以让 Agent 正确填写。
- 只映射完成任务所需的参数，不把内部或无关参数泄漏给 Agent。

参数说明应回答“这个值在当前任务中代表什么”，必要时说明格式、单位、稳定标识来源或与其他字段的关系。不要把多字段组合规则重复塞进每个参数；此类不可拆分规则归 `full_description`。

## Group and Binding

- Group 的层级完全由 `path` 表达，不新增 `parent`、`group_kind` 或计数字段来重复建模。
- Binding 通过 `group_path` 指向挂载位置；优先绑定已有 Tool record。
- `display_alias` 只解决同一 Tool 在特定任务入口中的用户语言差异，不用它制造多个能力身份。
- `visible=false` 或禁用 Tool 不计入 Agent 可发现能力。
- `children_count` 根据启用子 Group、可见 Binding 和启用 Tool 实时计算；Tool 自身为 `0`。

## Reuse Compatibility

Tool record 是跨 Binding 共享的 Agent contract，Binding alias 只能改显示名称，不能覆盖 short/full description 或 mapping。复用必须同时满足：

1. execution target 与参数、结果 contract 等价；
2. Tool 名称和描述对所有入口都如实成立；
3. Agent-facing 参数名、默认前提与组合说明一致。

若通用 interface 已被包装成 “Workflow 专用 Tool”，不能只改 Binding alias 后挂到 Agent Flow。优先把共享 Tool 改成对全部消费者都真实的通用 contract；若两边确有不同默认前提或说明，则分别创建领域 Tool，不把“去重”误解为“一条 interface 只能有一个 Tool”。

## Mapping Containers

接口 wrapper 会依据当前 request Schema 递归物化 `path`、`query`、`body` 中缺失的 required object container。mapping 仍只表达有真实业务含义的 Agent 输入：

- 不为 `mapping.output` 之类必需空对象增加虚假 selector、父容器参数或占位值。
- 用真实 `mcp.call` 验证请求已经越过 `request_schema` 并到达目标业务边界；保存成功、明确业务拒绝或 `target_interface/http_status` 都可作为到达证据。
- present `""` 与 `null` 必须原样交给 JSON Schema 判断是否合法；只有路径不存在才属于 mapping required 缺失。
- 若 current `mcp.get` Schema、保存 mapping 与运行结果仍不一致，按运行时缺口停止依赖该 Tool，不用宽松 Schema 或噪音字段绕过。

## Output Mapping

`result_schema` 描述结果结构，`output_mapping` 只描述确有必要的结果投影，两者不是同一个字段：

- 无需重命名、挑选或重组结果时，保存 `output_mapping: {}`。
- 不把完整 JSON Schema 复制进 `output_mapping`；这种 schema-shaped mapping 可能保存成功，但会让调用阶段错误解释结果。
- 只有当前 mapping contract 已从源码或既有正确 Tool 得到证据时才配置投影；不得为规避 response validation 自造 mapping DSL。

## Dynamic Interfaces and Atomic Children

部分资源在生命周期切换后才注册动态接口，例如已发布 Data Model 的 CRUD。先创建并发布父资源，再重新读取 interface catalog 和生成的 OpenAPI；动态接口未出现前不创建假 Tool。动态 Tool 使用稳定业务路径挂载，每个执行 contract 只保留一个 canonical Binding。

部分创建接口会原子生成默认子资源，例如 single 模式页面的默认 Tab。父资源创建后先读取响应或详情，复用已经存在的稳定标识；只有 contract 明确没有创建子资源时才调用子资源创建 Tool。

若写操作副作用已经成功，但 MCP 因失真的 `response_schema` 返回 `invalid_tool_configuration`：

1. 不重复执行同一写操作；
2. 用独立 list/options/detail、运行态 capability 或 OpenAPI 回读稳定标识与真实状态；
3. 把该写 Tool 标记为运行时缺口，不用宽松 Schema、伪造 output mapping 或重复记录换取成功；
4. 不依赖该错误响应的独立路径可继续装配。

请求的 scope、运行态回读 scope 与物理资源归属必须一致。即使记录带有 workspace `scope_id`，若模型回读为 system 或物理资源使用 system 命名，也不能宣称 workspace 模型验收通过；保留成功实体并报告产品或运行时语义偏差。

## Invocation Contract

发布成功不自动意味着存在可绑定的 MCP 调用能力。只有 interface catalog 提供当前 operation，且认证、凭据、请求与结果 contract 都能从源码和运行态验证时，才创建 invocation Tool。publication `operation=null`、必需 application credential 缺失或仅有无法绑定的通用 HTTP 入口时，保留发布、状态和运行观测能力并报告调用缺口，不伪造执行目标。

## Registered Capability and Execution Model

应用级模型注册与节点执行配置是两个 contract。注册项可以只表达 capability、表单或可用模态，不保证包含 LLM 节点执行所需的 `provider_code`、`model_id` 或协议参数：

- 先按节点当前 Schema 核对执行字段，再从可验证的 provider/model 来源取值。
- 不把 registration 名称、展示名或 capability 列表直接填入 LLM 节点执行字段。
- 模型目录 Tool 不可调用时，可只读取用户明确提供的参考应用验证当前 contract，但不得修改参考应用，也不得把一次性模型标识写入 Skill。
- 验收以真实单点或整链 trace 中的 provider、model 与 protocol 为准，不以保存草稿成功代替执行证据。

## Discovery Policy

设置能够支持当前 Virtual UI 深度的最小 `list_max_depth`，并为正常探索提供合理的 default limit。关键词正则与返回字段只影响发现体验，不承担权限控制。除非有可验证的上下文或性能问题，不做激进收缩。

## Minimal Difference

按以下顺序减少重复与风险：

1. 修正文案或 mapping；
2. 移动或更新 Binding；
3. 复用已有 Tool 并新增必要 Binding；
4. 仅在没有等价能力时新增 Tool；
5. 仅在导航确有分流价值时新增 Group；
6. 仅在探索行为无法满足路径时调整 discovery policy。

## Failure-Safe Assembly

维护本轮 created/updated/reused 账本，并按以下顺序装配：

1. 用一个代表性简单 Tool 验证创建或更新链路；
2. 创建或更新全部 Tool，并逐个 `mcp.get`；
3. Tool 就绪后创建 Group；
4. 创建 Binding 并用 `mcp.list` 核对 `children_count`；
5. 最后调整 discovery policy 并执行真实调用。

中途停止时，不保留没有可调用 Tool 的空 Group、语义不兼容的复用 Binding 或无消费者的新 Tool。只回滚本轮创建且确认无其他消费者的记录；已有记录和并发变化不得擅自恢复旧值。

候选 Tool 因 `response_schema`、不可绑定 interface 或独立 contract 缺口失败时，先回滚该 Tool 与 Binding，再继续不依赖它的能力；不要让一个目录查询能力阻断整个领域生命周期。
