# Composition Contract

## Scenario

用户希望 Agent 创建一个由多个节点和连接关系组成的工作流。每个节点有自身配置，连接关系引用节点稳定标识，部分节点还引用先前创建的制品。

## Reasoning

1. 从工作流 GUI 提取真实顺序：建立工作流容器、定义节点、连接节点、保存或发布。
2. 从 backend/interface catalog 确认是一个支持组合输入的接口，还是多个必须按顺序调用的接口；不能从页面按钮数量推断。
3. 若必须多次调用，分别建立可执行 Tool，并在 Virtual UI 中按用户阶段组织；不要伪装成一个不存在的组合接口。
4. 每个字段的名称、类型、必填性和单字段含义写入 input mapping。
5. “连接器只能引用本次请求中节点的稳定标识”“引用制品必须先存在”等不可拆分关系会影响正确调用，且不能由单字段 description 完整表达，因此写入精炼的 `full_description`。
6. 不写节点内部实现代码；只保留 Agent 构造正确请求所必需的组合契约。

## Good Full Description

```text
节点先声明稳定标识，连接关系再通过该标识引用起点和终点；引用外部制品时，制品必须已存在且属于当前工作区。节点数组顺序不代表执行顺序，执行关系由连接定义。
```

## Bad Full Description

```text
用于创建工作流。后端会遍历节点数组并调用若干 service，然后写入数据库。
```

前者补足跨字段、状态和制品关系；后者只是短描述与内部实现噪音。

## Acceptance Focus

- Agent 能通过 `list/get` 找到正确阶段与所有必要 Tool。
- `get` 暴露的 mapped Schema 与组合说明一致。
- `call` 的最小合法组合成功，错误节点引用被后端明确拒绝。
- `full_description` 只出现在确实存在不可拆分组合契约的 Tool 上。
