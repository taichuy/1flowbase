# Simple Capability

## Scenario

用户希望 Agent 查询某个会话的基本信息。GUI 中这个查询结果可能在列表、详情抽屉和调试页重复出现，但后端只有一个等价查询 contract。

## Reasoning

1. 以“查看会话信息”为用户目标，不按三个 GUI 组件创建三个 Tool。
2. 从 GUI 取得“会话”术语和进入顺序，从 interface catalog 确认稳定会话标识、返回字段与权限。
3. 在现有 catalog 中寻找等价 Tool；存在则复用并修正 Binding 或说明。
4. 使用最浅且无歧义的路径，例如 `/conversations/query`；若 `/conversations` 已足够分流，不增加“详情页”等展示层。
5. `short_description` 写直接结果，例如“返回指定会话的基本信息”。
6. 参数 description 说明稳定会话标识从哪里取得；这是单字段语义。
7. 没有跨字段或跨状态组合契约，因此 `full_description` 必须是 `""`。

## Expected Shape

```text
/conversations
└── query
    └── get_conversation
```

这只是投影模式，不是固定应用 catalog；实际名称、路径与接口必须从当前源码确定。

## Acceptance Focus

- `mcp_list` 只出现一个 canonical 查询能力。
- `mcp_get` 的参数名和 description 足以构造调用。
- `mcp_call` 返回与后端 contract 一致的会话信息。
- 相同 GUI 组件职责没有造成重复 Tool 或无意义的非空 `full_description`。
