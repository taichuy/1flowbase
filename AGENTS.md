# 记忆
命中对应`记忆存储规则`自动更新对应记忆中
@.memory/AGENTS.md
## 用户偏好
`.memory/user-memory.md` 是本地私有文件，不提交；缺失时参考 `.memory/user-memory.template.md` 初始化。

# 本项目相关skill在
.agents/skills 是项目 skill 源目录,其他skills只作为本地镜像
如果没有注册，请自行更新到对应约定目录

# 本项目 skills
1.`problem-framing`：需求对齐、会导向产品决策的功能 / 交互诊断、方案拍板与计划形态选择；计划只使用 Single Issue 或两层 Issue Tree。
2.`frontend-development`：前端页面、UI 结构、工作区流程、节点开发、schema UI、交互和视觉结构变更时使用。
3.`backend-development`：后端 API、状态流转、模块边界、核心业务逻辑、状态写入口和一致性设计变更时使用。
4.`test-driven-development`：功能、缺陷、重构或行为变化可用自动化测试覆盖时，在实现前使用。
5.`qa-evaluation`：进入自检、验收、回归、交付或质量评估阶段时使用，输出证据驱动的 QA 结论。
6.`github-solution-research`：具体工程问题可能已有 GitHub 开源证据或方案时使用；搜索 issue、PR、repo、code，比较候选项目并提炼本地适配与验证计划。

# 质量控制
1.进入自检、验收、回归或交付阶段时，使用skill `qa-evaluation`；
2.前端实现规则: `web/AGENTS.md`
3.后端实现规则: `api/AGENTS.md`
4.warning 与 coverage 产物统一落到 `tmp/test-governance/`。
5.涉及功能、缺陷、重构或行为变化的开发，先使用项目 skill `test-driven-development`；若不适用，交付说明需写明原因与替代验证。
6.后端是唯一数据来源，前端不应该作代码处理输出兼容，应该是后端提供职责单一的接口
7.前后端接口字段名必须与后端 DTO / 领域语义保持一致；UI 展示名可以本地化，但不得为展示文案另起接口字段别名。确需兼容旧字段时，必须在兼容代码最近行标记 `@field-contract-compat source=... alias=... remove_by=yyyy-mm-dd`，带废弃计划和测试，并让 `repo-hygiene` 以 warning 暴露。
8.前端多语言资源必须让 `i18n-hygiene` 暴露无静态引用 key；`unused-i18n-key` 是 warning，保留时必须说明动态 key、路由配置或外部渲染入口原因。
9.受保护页面和认证 API 的本地运行态取证使用 `page-debug` / `api-debug`；临时脚本不得直接调用 `/api/public/auth/sign-in`。确需自定义 Playwright 流程时，复用 `scripts/node/page-debug/auth.js` 的临时 session owner，并在 `finally` 回收。

# 开发流程控制
1.需求类请求，以及会导向产品决策的功能差异、缺失能力、不可编辑状态或入口归属诊断，默认先使用 `problem-framing`；方向确认前不修改产品代码。
2.普通任务使用 Single Issue；跨上下文、多 agent、跨仓或包含多个可独立集成结果的长计划使用 Root → Delivery 两层 Issue Tree。Root 一次批准既定 Delivery，用户只验收 Root。
3.只有不影响产品 / 设计决策的事实查询、机械精确改动，或用户明确要求直接开始 / 无需确认时，才可跳过需求对齐；“为什么流程不一致 / 能力缺失 / 无法编辑 / 应放在哪个入口”不属于纯查询。
4.长计划执行必须读取 `.agents/skills/problem-framing/references/long-running-work.md`；默认先用一个只读 Scout，Root agent 汇总路径、现状、目标、AC 与集中 Test Batch，并成为唯一 packetizer、assembly owner 和 Control Ledger owner。
5.开发只接收边界明确的 Work Packet；同一 Delivery 可连续复用开发上下文，新 Delivery 使用最小 handoff。开发、reviewer、QA agent 不再嵌套调度 agent。
6.Root 下全部开发与 fixture Work Packet 装配到隔离 assembly 后，只启动一个 fresh QA 做集中测试；不做 per-packet / per-Delivery QA。无依赖、无共享写 / contract / migration / 端口 / 构建冲突的开发 Packet 才可并行，状态与资源检查保持事件驱动。

# 文件管理约定
1.理论上来说单个代码文件不应该超过1500行
2.当前单个目录下文件不应该超过15个，超过后应该收纳整理对应子目录
3.测试文件默认放到对应子目录下的 `_tests`；更深层 `AGENTS.md` 对集成测试等场景另有规定时，以更深层规则为准。
4.如果对应子目录下有AGENTS.md，需要先介绍阅读再做处理
5.所有AGENTS.md，目标是提供短、硬、稳定的本地执行规则，尽可能精准，清晰，简短，最多不得超过200行。
6.`docs/superpowers/plans` 和早期 `docs/superpowers/specs` 属于历史计划/规格归档，允许按时间保留旧文件；引用前必须优先核对最新 AGENTS、README 和 superseded 标记。

# 规则编写约定
新增或调整 AGENTS / skills 时，优先写目标、验收证据、资源边界和停止条件；绝对词只用于真不变量，不把可判断事项写成冗长固定流程。

## Command Output

Protect context usage. **Any command with unknown or potentially large output must be byte-capped.**

Default pattern:

```bash
COMMAND 2>&1 | head -c 4000
```
