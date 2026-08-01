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
| `output_mapping` | 接口结果到 Agent 结果的必要投影 | 隐藏真实错误或伪造成功 |
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
