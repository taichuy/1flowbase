# Examples

示例只说明输出尺度，不是固定答案或字段清单。优先根据真实证据自主分析。

## UI Or Flow Requirement

用户：“优化首次进入工作区的引导。”

```text
[进入工作区]
      |
      +-- 已有内容 --> [正常工作区]
      |
      +-- 空工作区 --> [说明] -> [主操作] -> [可见结果]
```

在三个方向中比较引导强度、用户中断成本和状态归属；ASCII 图只表达主路径。

## Diagnostic-Looking Requirement

用户：“为什么新增能配置定时器，编辑却不能，也看不到扩展接口？”

这仍会改变产品入口、contract 与 owner，不是可跳过的事实查询。先取证新增、编辑、后端 DTO 与运行入口，再完整输出 `现状`、`需求分析`、三个方向和 `最终建议`；即使根因已确定，也不以“结论”开头或省略方向。

## High-Risk Contract Requirement

用户：“统一 frontend/backend defaults。”

先读取 `domain-matrix.md`，区分前端展示 fallback、后端默认值、已落库设置和 runtime contract。三个方向都必须说明 source of truth、历史数据影响和验收证据；不要因为名称相同就假设它们是同一概念。

## Plan Shape Selection

普通任务即使同时修改 API、状态和 UI，只要能由一个连贯结果闭环，仍使用 Single Issue。例如“列表支持服务端排序并在当前页面展示”只有一个用户结果，不按 frontend/backend 拆树。

长计划只有在存在多个可独立集成结果时使用 Issue Tree。例如 AI Gateway V3 可以按纵向结果组织：

```text
[Root：V3 可上线并可回滚]
  ├─ [Delivery：Generate 端到端]
  ├─ [Delivery：Compact 与 context transaction]
  └─ [Delivery：双栈 rollout 与 rollback]
```

“先定义类型 → 再写 mapper → 再做 storage → 再补测试”是实现步骤，不是 Delivery Map。
