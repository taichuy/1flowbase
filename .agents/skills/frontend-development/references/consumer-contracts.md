# Frontend Consumer Contracts

## Scope And Authority

适用于 API client、权限目录 / 角色编辑、请求取消和流式 UI；只读当前任务命中的章节。这里规定前端如何消费已确认 contract，不决定新接口、授权策略或取消语义。

- 接口授权与注册归属：[后端设置注册规则](../../backend-development/references/console-settings-registration.md)。
- 协议、身份、终态与交付：[接口生命周期](../../../../docs/architecture/interface-lifecycle.md)，按文档目录读取相关章节。
- 当前角色目录与写入类型：[console-roles.ts](../../../../web/packages/api-client/src/console-roles.ts)；类型是消费入口，后端 DTO / 已确认 contract 才是语义真值。

## API Consumption

- `api-client` 负责 DTO、transport、协议解析；feature 请求层负责 query key、query/mutation 和消费编排；组件负责展示与交互。共享层按真实复用提取，不为单次调用增加转发层。
- 前端可维护选择、展开、表单草稿、加载与连接状态，也可按 contract 格式化展示；不得补造后端缺失的业务字段、权限、运行结果或错误原因。
- 缺字段或契约互相矛盾时保留缺口证据，交给对应后端 owner；不通过默认值、字段别名、局部缓存或旧协议 fallback 掩盖。旧测试不能单独构成兼容要求。

## Settings And Authorization

- `feature_id` 标识后端注册归属；角色可配置 console operation 保持 `1 operation ↔ 1 method + route template`。UI 使用 catalog 的 `operation_id` 和 `route`，不根据 feature、页面 URL、菜单分组或 HTTP 动词自行生成授权集合。
- 前端 route guard、菜单和按钮消费后端权限结果，只影响导航 / 交互；隐藏入口不能替代服务端授权。缺失权限结果不能默认放行，也不能把网络失败伪装成空权限目录。
- group 的 `enabled` 与 `strategy: full | custom` 是独立维度；关闭 group 不清空已有自定义 operations，切换 full/custom 不借机重置自定义配置。按后端 catalog 的 `full_profile`、可选 scope 和 policy contract 展示或编辑，不自行扩权。
- operation 的 `summary`、`description` 由 API owner 提供并经 interface catalog 投影；前端原字段消费，不添加 operation `label_ref / description_ref`，不从路由猜说明。group 文案、通用按钮与 API 说明分别遵守各自 DTO / i18n owner。
- 插件 inactive 或权限拒绝时保留后端规定的角色配置语义；不因入口暂时不可用删除 policy，也不以页面可见性绕过 workspace、row、field 或 secret 约束。

## Streaming And Async State

```text
停止监听 / AbortController / 连接断开 → 本地消费或交付状态
显式取消 API                        → 请求业务取消
后端业务事件 / 状态查询             → 已确认业务状态
```

- 分开管理连接状态、取消请求状态和后端业务状态；不能把 EOF、socket close、组件卸载或请求超时直接写成业务 completed / cancelled / rolled_back。
- “取消请求已接受”不证明业务已取消；收到既有 contract 的终态或查询结果后再更新确认状态。不新增虚构的取消协议，也不宣称远端 provider 已停止计算。
- 断流后按既有接口恢复观察或查询；没有对应能力时明确状态尚未确认并反馈契约缺口。不要自动重放可能已提交的 mutation，重试遵循该接口已有的幂等与重试语义。
- 订阅、请求和清理归拥有它们的消费层；切换对象、scope 或卸载后，旧响应不能覆盖新上下文。清理前端资源不等于发起业务取消。
- HTTP / SSE / WebSocket 保留各自输入、事件、错误与终态包装；UI 不用“统一响应”丢弃协议语义。后端允许透传的 provider 错误保留排障信息，普通交互反馈与正式诊断内容按既定产品 contract 展示。

## Evidence And Stop Conditions

- 角色编辑选择本次涉及的保存 / 重载、group 开关、full/custom 和单 operation 场景；UI consumer test 证明编辑语义，API 授权正反例交给后端测试 / QA，不能互相替代。
- 流式变更按风险选择正常终态、断流无终态、取消被拒绝 / 接受但未终态、切换对象的迟到事件；纯静态页面不加载这组测试。
- DTO、交互与已确认预期一致，直接消费者风险已有证据时停止。若需要定义新业务终态、授权范围或恢复协议，回到 `problem-framing`；既定边界内的局部实现继续推进。
