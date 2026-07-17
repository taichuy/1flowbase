# Long-Running Work

用于 Issue Tree、多 agent、跨上下文或跨仓库的长计划。目标是持续减少 Root 验收风险，而不是最大化并发、提交数或子 issue 完成数。

## Control Model

把长计划视为离散反馈系统：

```text
Root 目标与 AC
      ↓
选择下一纵向结果
      ↓
实现并进入唯一集成基线
      ↓
端到端证据与风险变化
      └──────────────→ 更新 Control Ledger
```

选择能以合理成本最大幅度减少 Root 验收残差和关键不确定性的 Delivery。foundation、类型、测试、文档、commit 或评论只有被可运行结果消费后才计入进展。

## Root Control Ledger

Root 保存长任务压缩后的最小状态：

```md
## Control Ledger
- 最终结果：
- 硬约束与授权边界：
- 当前已集成基线：
- 已结算 / 未结算 Root AC：
- 当前 Delivery 与预期证据：
- 已知阻塞、不确定性和外部扰动：
- 活动工作树及所有权：
- 下一可控结果：
- 停止或重构条件：
```

每次集成、关键失败或计划变化后更新账本。跨上下文继续时读取账本、当前代码和证据，不重放完整对话或复制所有历史命令。

## Delivery Design

- 以用户或系统可观察结果纵向切片，穿过所需的 API、domain、runtime、storage、provider 或 UI。
- 每个 Delivery 映射至少一个 Root AC，并能安全进入集成基线。
- 共享 foundation 由最早消费它的 Delivery 拥有；后续 Delivery 从已集成入口扩展。
- 依赖图只表达结果依赖。文件清单、编码步骤和测试命令属于 handoff，不创建新 issue。
- 优先建立最短端到端闭环，再扩协议、Provider、性能和覆盖面；没有稳定闭环时不并行铺开多个水平基础层。

## Delegation

subagent 提示只提供完成任务所需的高信息字段：

```md
Outcome:
Root AC:
Integrated baseline:
Scope and ownership:
Authority and hard constraints:
Required evidence:
Stop or escalate if:
```

不要重复仓库已有 AGENTS / Skill 规则，不规定模型可自行推导的流程，不用示例堆叠同一要求。优先延续熟悉同一交付域的开发 agent；最终 QA 使用全新上下文。

## Concurrency And Integration

- 只并行不存在写入冲突、端口冲突或隐式 contract 依赖的 Delivery 工作。
- 默认同时保留不超过两个未集成开发 Delivery；出现更多局部成果时优先集成和取证，而不是继续开工。
- Root agent 拥有唯一集成基线、Control Ledger 和范围判断；开发 agent 只拥有明确 worktree 与交付边界。
- 局部分支绿灯不是完成。合并冲突、集成回归和 Root AC 证据由集成基线判断。
- 重型验证按收益和风险集中运行；同类编译、服务或 QA 不并发争抢资源。

## Evidence And Checkpoints

- 进展用已集成 Root AC 证据衡量，不用代码行数、commit、issue 评论或局部测试数量衡量。
- 每个 Delivery 完成后立即更新 Root 账本、证据链接、剩余风险和下一结果。
- 两次连续交付仍未减少 Root AC 残差，或同一阻塞重复出现，应停止扩张并重新分析关键路径。
- 已有证据足以结算风险时停止追加等价验证；代表性 fixture、beta/CI 和最终 QA 各自承担明确证据层级。

## Stop Or Reframe

出现以下任一情况时停止当前 Delivery，回到 Root / `problem-framing`：

- 目标、Root AC、source of truth、用户内容、migration 或权限边界需要改变。
- 需要新增未获批准的 Delivery 或跨出当前授权范围。
- 两份计划、两条运行真值或两套状态开始并存。
- 集成基线无法吸收局部成果，或继续并行只会扩大冲突和未验证状态。
- 资源接近项目红线、外部系统状态不安全，或完成需要新的外部授权。

所有 Delivery 集成后只启动一个全新 QA 上下文。QA 失败则回到对应 Delivery 修复，再用新的 QA 上下文重验；Root 只在证据完整且用户验收后关闭。
