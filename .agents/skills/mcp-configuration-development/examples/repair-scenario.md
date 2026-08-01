# Repair Scenario

## Scenario

现有 MCP catalog 中，同一个能力因 GUI 多处入口被创建了多个 Tool；每个 Tool 的 `full_description` 都重复 short description 和 HTTP path，Agent 参数仍使用后端内部名称，Group 下的 `children_count` 也被误认为需要手工配置。

## Repair

1. 从当前 GUI 和后端源码证明这些入口是否共享同一执行 contract 与用户结果。
2. 选择一个稳定的 canonical Tool；保留仍被有效 Binding 引用且命名最符合领域语义的记录。
3. 将必要入口绑定到 canonical Tool；只有任务语义不同才保留多个 Binding。
4. 通过 input mapping 把内部参数改为 Agent 可理解的名称，补充参数 description 和 required。
5. 将普通 Tool 的 `full_description` 清为 `""`；组合 Tool 只保留不可由其他字段表达的契约。
6. 删除或禁用无引用的重复配置时，先确认没有其他实例依赖；不进行超出当前任务的批量清理。
7. 不新增 Group 计数字段，也不写入 `children_count`；验证运行时派生结果。

## Before

```text
/apps/list/details/get_item_v1
/dashboard/recent/get_item_v2
/debug/http/GET_item/get_item_v3
```

## After

```text
/items/query/get_item
```

路径仅说明去重方式，真实路径必须从当前用户任务与 catalog 确定。

## Acceptance Focus

- 关键词和路径探索只暴露有证据的 canonical 入口。
- `mcp.get` 不再包含重复长说明，mapped input Schema 使用 Agent-facing 参数。
- `mcp.call` 仍执行同一个后端 contract。
- Group `children_count` 与修复后的可见子项实时一致。
