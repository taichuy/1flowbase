# Source Routing

## Evidence Order

按问题选择真值源，不要求一个文件回答所有问题：

| 问题 | 首要证据 | 交叉验证 |
| --- | --- | --- |
| 用户为什么执行、从哪里开始 | `web/` 路由、页面、交互动作、i18n | 产品状态与相邻流程 |
| 能否执行、需要什么输入 | backend interface catalog、route、DTO | service/domain validation |
| 执行后得到什么 | response DTO、domain result、测试 | GUI 成功态与结果展示 |
| 权限、风险、状态限制 | interface descriptor、permission code、domain rules | route middleware、失败测试 |
| 当前已经配置什么 | MCP catalog/API/管理界面读模型 | 实例、Group、Tool、Binding、policy 分项读取 |
| Agent 实际能看到什么 | `mcp_list`、`mcp_get` | 协议路由与运行时测试 |
| Agent 实际能否调用 | `mcp_call` | 后端持久化结果或可观察副作用 |
| MCP 调用失败 | JSON-RPC `category`、`field`、`http_status` | 本地结构化日志、失败 Tool、interface 与阶段 |

## Search Strategy

1. 用 GUI 中的用户术语搜索 `web/`，定位路由、页面和动作处理器。
2. 沿前端 API client 找到后端 route 与请求/响应字段。
3. 沿 route 找到 interface registration、DTO、service 和 domain validation。
4. 在 interface catalog 中按当前 descriptor 确认 `bindable`、参数、Schema、权限与风险。
5. 再用目录列表和单 Tool 读取比较目标能力是否已存在、已挂载或配置错误；完整 catalog 只用于规模可控且确需全局引用关系的场景。

优先使用 `rg` 和源码引用。不要仅凭文件名、界面截图、旧文档或记忆推断 contract。

## Conflict Resolution

- GUI 与后端字段名不一致时，以后端 DTO/领域语义作为接口字段真值，GUI 文案只用于显示名称与任务语言。
- GUI 展示了动作但 interface catalog 不可绑定时，记录业务接口缺口，不创建不可执行的假 Tool。
- interface catalog 有能力但 GUI 没有对应用户目标时，不自动暴露；先确认它是公开任务还是内部基础能力。
- 已保存配置与当前源码冲突时，以当前可验证 contract 为目标，保留最小修复差异并说明迁移影响。
- 运行时输出与保存配置冲突时，记录运行时缺口并停止依赖该行为的批量配置。
- 优先使用 JSON-RPC `category`、`field` 与可选 `http_status` 区分参数、Tool 配置、请求/响应 Schema、interface catalog 和目标业务边界。只有返回 `interface_dispatch`、通用错误或分类与证据冲突时才读取本地结构化日志；无法取得证据时标记未分类运行时缺口。

## Scope Control

每轮只处理约定领域或用户任务。不要为了“完整”遍历并重建整个应用 catalog。发现相邻缺口时加入覆盖表，但只有它阻断当前任务时才进入本轮配置差异。
