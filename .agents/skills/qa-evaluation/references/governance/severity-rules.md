# QA Severity Rules

| 严重度 | 含义 | 默认处理 |
| --- | --- | --- |
| `Blocking` | 当前问题会阻塞继续推进、交付或合并 | 必须先修复 |
| `High` | 当前问题风险高，默认必须修复 | 必须修复 |
| `Medium` | 当前问题不是阻塞，但已足够影响质量和可信度 | 默认应修复 |
| `Low` | 当前问题影响较小，但仍是质量债务 | 默认也修复；若暂缓必须说明原因 |

## Severity Hints

| 场景 | 建议严重度 |
| --- | --- |
| 主路径走不通、关键结果错误、核心状态不一致 | `Blocking` |
| 改共享组件导致其他消费者行为变化 | `High` |
| 前后端字段契约漂移，或为了展示文案新增未标记业务字段别名 | `High` |
| 旧字段兼容已标记 `@field-contract-compat`，且有废弃计划和测试 | `Low` warning |
| 局部交互错位、映射不一致、回归缺口明显 | `Medium` |
| 无消费者小导出、过期注释、低风险废弃分支或命名含糊但未影响主路径 | `Low` warning |
| 文案、边角一致性、小范围低风险瑕疵 | `Low` |

## Backend Severity Hints

| 后端场景 | 建议严重度 |
| --- | --- |
| 绕过 service 直接改关键状态、插件可注册系统接口、`public / control / runtime` 混层 | `Blocking` |
| 公共 API 契约变化未回归、`RuntimeExtension / CapabilityPlugin` 边界被打破、repository 混业务规则、`Resource Action Kernel` 托管边界失效 | `High` |
| HostExtension 绕过 manifest contribution 直接扩系统接口、直接改 Core 真值表，或 RuntimeExtension / CapabilityPlugin 直接持有 Redis、NATS、RabbitMQ 等基础设施连接 | `High` |
| `storage-durable/postgres` 内的 `storage-postgres` repository / mapper 拆分被打回混层实现、`storage-durable / storage-object` 边界被混用、mapper 藏业务规则、dynamic modeling 与 runtime data 被混成同一层 | `High` |
| `ApiSuccess` / `204` / 错误结构不一致、后端验证命令或验证脚本缺失、测试目录或命名不对齐 | `Medium` |
| 文档、命名、低风险一致性瑕疵，但未直接影响行为 | `Low` |

## Frontend Severity Hints

| 前端场景 | 建议严重度 |
| --- | --- |
| 主路径可用，但共享样式递归覆盖打坏原生组件布局或交互 | `High` |
| 共享壳层或第三方组件覆写未做其他消费者回归 | `High` |
| 导航文案、`route id`、`path`、选中态不一致，用户路径真相被写散 | `Medium` |
| UI 质量没有真实证据，只按代码主观判断通过 | `Medium` |
| 目录边界失焦、测试目录未对齐、文件职责持续膨胀 | `Medium` |
| 低风险视觉一致性瑕疵，但不影响主路径和原生组件行为 | `Low` |

## Maintainability Severity Hints

| 可维护性场景 | 建议严重度 |
| --- | --- |
| 空转抽象污染公共 API、状态入口、数据一致性、前后端契约、插件边界或核心写路径 | `High` |
| 局部 wrapper / helper / service 无真实职责，已经增加理解成本、测试成本、blast radius 或重复修改概率 | `Medium` |
| 无消费者的小导出、过期注释、低风险废弃分支、命名含糊但暂未影响主路径 | `Low` warning |
| 只有命名直觉、没有调用方 / 边界 / 运行路径 / 历史证据 | `未验证`，不下确定结论 |

## Reporting Rules

- `Medium` 及以上问题默认都应进入修复范围
- `Low` 不等于可以忽略，只是优先级最低
- 严重度判断必须基于证据和影响，不要只看改动大小
- 证据不足时不要强行给高结论，应该明确写未验证限制
- maintainability warning 不是修改授权；删除、合并抽象、改公共接口或清理兼容层前必须等待用户同意
