# Virtual UI

## Definition

Virtual UI 是面向 Agent 的任务导航层：人通过 GUI 页面逐步发现并操作应用，Agent 通过 MCP Group、Tool 契约和调用结果逐步发现并操作同一应用。

```text
人     → GUI 页面 → 页面动作 → 后端执行
Agent  → MCP Group → Tool     → 后端执行
```

两条路径共享用户目标、领域术语、状态语义和后端约束；它们不需要共享组件数量或展示布局。

## Projection Rules

1. 从用户目标开始，不从数据库表、HTTP path 或 React 组件名开始。
2. 按 GUI 中真实任务的先后关系组织 MCP 层级，使 Agent 能从领域入口逐层缩小范围。
3. 将多个页面或组件中出现的同一业务能力合并为一个 canonical capability。
4. 只有任务语义确实不同，才允许同一 Tool 出现在多个 Group；展示位置不同不构成充分理由。
5. Group 表达导航领域，Tool 表达可执行动作，mapping 表达 Agent 输入与接口输入的转换。
6. discovery 负责让能力容易找到，不复制 GUI 的菜单权限或按钮显隐。
7. 权限、资源归属与状态合法性由后端执行入口判断；MCP 配置不得绕过，也不需要提前隐藏所有可能失败的能力。

## Canonical Path Test

对每个候选入口依次提问：

- Agent 的目标是什么？
- 这个层级是否帮助缩小任务范围？
- 删除该层级后是否仍能无歧义找到能力？若能，删除它。
- 另一个入口是否代表同一个执行契约？若是，合并或复用。
- 该名称是否来自用户可理解的领域语言？若不是，回到 GUI 与业务源码取证。

## Depth Heuristic

路径深度不是 GUI 页面层级的复制。使用满足无歧义发现的最浅层级：

```text
领域 → 用户对象或阶段 → 动作
```

若“对象或阶段”没有实际分流作用，直接使用“领域 → 动作”。路径过浅导致能力混杂时再增加一层，不为视觉整齐增加空目录。

## Discovery Behavior

- 默认入口应能让未知应用结构的 Agent 看到主要领域。
- `mcp.list` 返回简短结果与 `children_count`，用于决定是否继续下钻。
- 关键词搜索补充路径探索，但不能替代清晰的 canonical path。
- `mcp.get` 才承载完整 Tool contract；不要把所有细节堆进列表描述。
