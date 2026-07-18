# Examples

示例只说明输出尺度，不是固定答案或字段清单。优先根据真实证据自主分析。

## Ordinary Requirement

用户：“列表增加可选排序。”

```md
## 现状
当前列表只有固定顺序；是否改变默认顺序尚未确认。

## 需求分析
真正需求是让用户按场景调整查看顺序，同时保护现有默认行为。

## 三个方向
### 保守
- 方案内容：增加一个局部排序入口，不改变默认排序。
- 综合收益：范围最小；扩展性有限。

### 平衡
- 方案内容：建立明确的排序字段与方向，由当前列表模块统一处理。
- 综合收益：语义清楚、可扩展；需要补齐状态与验收。

### 激进
- 方案内容：建设可复用的列表查询与排序体系。
- 综合收益：长期统一；当前需求可能承担过多成本。

## 最终建议
采用平衡方向；请确认作用页面和默认排序保持不变。
```

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
