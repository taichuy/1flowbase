# QA Evaluation Modes

## Selection Rules

| 模式 | 什么时候进入 | 必要输入 | 默认输出 |
| --- | --- | --- | --- |
| `task mode` / `Dev Acceptance Gate` | 用户要验证当前任务、当前改动、某个局部版块，或功能开发完成后需要快速验收 | 任务目标、改动范围、验收场景、相关页面 / 模块 / API 边界、TDD 红绿结果 | 局部问题报告、已验证 / 未验证项和残余风险 |
| `PR gate mode` / `PR Merge Gate` | 用户要求 PR 校验、合并前门禁、贡献者质量反馈或 CI 结果解读 | PR 分支 / commit、GitHub Actions run 或本地替代命令、artifact / warningFiles | blocker / warning / advisory / cost / 合并风险 |
| `project evaluation mode` / `Project Health Gate` | 用户明确要求“全量评估项目”“评估项目现状代码”“完整 QA 审计”“全量门禁体检” | 项目当前范围、目标分支 / commit、相关 spec、项目记忆、反馈记忆、远端 artifact 或本地全量证据 | 全量健康快照、风险热力图、趋势、轮转深挖和维护建议 |

## Default Rules

- 默认进入 `task mode / Dev Acceptance Gate`
- `PR gate mode` 和 `project evaluation mode` 只有用户明确授权才允许启动
- `task mode` 可以在当前会话运行，但要提示存在上下文偏置
- 更推荐在新会话中运行，以降低实现路径带来的宽容偏差
- 三种 gate lane 的资源预算、算法目标和停止条件读取 `gate-lanes.md`

## Companion Skill Routing

| 问题类型 | 需要联动 |
| --- | --- |
| 信息架构、入口层级、L0 / L1 / L2 / L3 深度关系 | `frontend-logic-design` |
| 前端页面语法、交互一致性、视觉系统约束 | `frontend-development` |
| 后端 API 契约、状态入口、边界污染 | `backend-development` |

## Hard Stops

- 用户没有明确要求时，不要把局部回归升级成全量项目审计
- 用户没有明确要求时，不要把开发后验收升级成 PR 门禁或项目体检
- 评估范围不清时，先收敛范围，再开始下结论
- 没有验收场景和边界输入时，`task mode` 只能给出受限结论
