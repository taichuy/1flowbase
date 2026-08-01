# Acceptance

## Evidence Checklist

### Source

- 任务范围有明确的 GUI 起点、用户目标和可观察结果。
- 每个 Tool 都能追溯到当前 bindable interface 与后端 contract。
- 参数名、必填性、结果、风险、权限和状态约束已交叉验证。
- 没有把旧文档、截图或静态能力清单当作唯一真值。

### Virtual UI

- 默认入口能发现目标领域，路径使用用户可理解的领域术语。
- 每层 Group 都有实际分流价值，没有复制纯展示组件层级。
- 相同能力已去重；重复 Binding 均有不同任务语义的证据。
- 简单任务不因追求整齐而增加多余层级。

### Configuration

- 优先复用已有 Tool，写入差异限制在当前任务范围。
- `short_description` 说明直接结果。
- 普通 Tool 的 `full_description == ""`；非空内容通过组合契约三条件检查。
- Agent-facing 参数名、说明和 required 与 `input_mapping` 一致。
- Group 与 Binding 引用有效，启用、可见与排序状态符合目标。
- 没有写入或配置 `children_count`。

### Runtime

按 Agent 的真实顺序保留请求和关键响应证据：

1. `mcp.list` 从默认入口或上级路径找到目标 Group/Tool。
2. `mcp.list` 的 Group `children_count` 与启用子 Group、可见 Binding 和启用 Tool 一致。
3. `mcp.get` 返回正确的 short/full description、mapped input Schema、result Schema、risk 和 description validation 信息。
4. `mcp.call` 使用 `mcp.get` 返回的 contract 成功执行，并验证可观察结果。
5. 至少验证一个关键失败边界，例如缺失必填参数、非法状态、无权限、错误稳定标识或错误 `des_id`。

## Coverage Table

交付时使用最小覆盖表：

| 用户目标 | GUI 证据 | Interface | Virtual UI path | Tool | list | get | call | 状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 目标描述 | 源码位置 | `interface_id` | `/domain/...` | `tool_id` | 通过/失败 | 通过/失败 | 通过/失败 | 已覆盖/缺口 |

## Defect Classification

- 配置缺口：Tool、mapping、Group、Binding、文案或 policy 可在当前 Skill 范围修复。
- 业务接口缺口：没有 bindable interface，或接口 contract 无法完成用户任务；报告并停止伪配置。
- 运行时缺口：保存配置正确，但 `list/get/call` 未按 contract 输出或执行；报告代码证据，不在本 Skill 中修复。
- 产品决策缺口：GUI 与后端表达不同目标或无法判断 canonical 行为；返回需求对齐。

## Completion Rule

只有目标能力同时满足“可发现、可理解、可调用、无无证据重复挂载”，且所有配置写入已重新读取确认，才标记完成。无法完成时给出最小阻断证据和下一责任边界。
