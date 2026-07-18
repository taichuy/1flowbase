# Repository Quality Gates

## Ownership

- 开发阶段不自动注入完整仓库质量门禁清单。
- 当前本地开发分支默认只做当前任务结果验证；仓库级、线上级、重型质量门禁默认在 beta / CI / 专门质量工作区运行和归档。
- 进入自检、验收、回归或交付阶段后，由 `qa-evaluation` 负责回答“当前范围该跑哪条门禁、是否需要组合、证据能否支撑 QA 结论”。
- 最近作用域的 `AGENTS.md` 只保留实现规则与少量不可变 QA 边界，不再枚举完整脚本清单。

## Heavy Gate Criteria

`重型质量门禁` 不按命令名字一刀切，按目的、范围、成本和运行态影响判断。命中任一条时，默认不作为 Dev Acceptance 本地收尾动作，除非已批准计划 / handoff 已前置说明收益、成本和不可延后原因。

- 目的：回答“仓库是否可合入 / CI 是否可过 / 项目健康度如何”，而不是“当前任务结果是否成立”。
- 范围：跨 workspace、全仓、全前端、全后端、全 coverage、全 hygiene，或检查与当前改动无直接调用链的大量消费者。
- 成本：会触发大范围编译、build、clippy、full test、coverage、security scan，或明显超过一个主验证命令和必要 smoke 的时间预算。
- 运行态影响：需要重启服务、真实认证链、外部中间件、运行态接口取证、写 `tmp/test-governance/` 全局 artifact，或可能干扰开发反馈节奏。

常见重门禁：`cargo test --workspace`、`cargo clippy --workspace --all-targets`、workspace 级 `pnpm build` / full lint / full test、`verify-repo`、`verify-ci`、coverage、repo hygiene、i18n hygiene、container / security scan、服务重启后 `api-debug` 取证。定向 crate test、route integration test、单消费者 contract test、局部 `tsc`、单路由 screenshot/page-debug 通常不是重门禁。

## Repo-Level Gate Map

| 命令 | 主要覆盖面 | 什么时候优先跑 | 不替代什么 |
| --- | --- | --- | --- |
| `node scripts/node/test-scripts.js [filter]` | `scripts/node` 自身脚本与编排逻辑 | 改了 Node 验证脚本、CLI 编排、治理脚本 | 不替代前后端业务回归 |
| `node scripts/node/cli/test-contracts.js` | 共享 DTO / consumer contract / style-boundary contract consumer | 改共享 console API DTO、settings / agent-flow provider consumer、`style-boundary` registry 或其他跨消费者契约 | 不替代页面质量和后端分层回归 |
| `node scripts/node/test-frontend.js fast` | 前端快速回归 | 需要仓库级前端快检，但还没到 full gate | 不替代移动端/真实运行态证据 |
| `node scripts/node/test-frontend.js full` | 前端 lint、test、build、style-boundary full gate | 要给前端结论兜底，或 `verify-repo` 前置确认 | 不替代具体页面走查和截图证据 |
| `node scripts/node/tooling.js gate-router [--staged]` | 根据 changed files 输出非阻塞质量门禁建议；提交前 hook 使用 `--staged`，线上 `repo-tooling` 使用 branch diff | 需要在开发阶段提示“本次改动应该跑哪些相关门禁”，但不想阻塞提交 | 不替代实际运行被建议的门禁 |
| `node scripts/node/tooling.js repo-hygiene` | 废弃标记、前后端字段兼容标记、弱断言、重复测试标题、超大文件和目录压力，报告写入 `tmp/test-governance/repo-hygiene.json` | 全量审计、热点预防、判断旧逻辑、字段兼容 alias 和测试重复是否已进入 QA 证据层 | 不替代业务正确性、覆盖率和人工架构审查 |
| `node scripts/node/tooling.js i18n-hygiene` | 多语言 locale 文件名、key 对齐、JSON 重复 key、同 owner value 重复和未引用前端 key warning，报告写入 `tmp/test-governance/i18n-hygiene.json`；`--include-cross-owner-warnings` 可额外输出跨 owner advisory | 改前端 / 插件 `i18n/`、语言切换、UI 文案抽取、common 文案或仓库级 QA tooling | 不替代人工判断跨 owner 文案是否真的同语义，也不替代动态 key 人工确认 |
| `node scripts/node/tooling.js check-rust-backend` | Rust 后端静态质量门禁，报告写入 `tmp/test-governance/rust-backend-static-gate.json` | 需要快速检查新增 Rust 后端坏味道，或定位 `test-backend` / `verify-backend` 前置静态门禁失败 | 不替代 cargo 编译、测试、clippy 和业务语义审查 |
| `node scripts/node/test-backend.js` | Rust 静态门禁 + 后端测试聚合入口 | 改后端实现，需要仓库根统一触发后端最小回归 | 不替代后端分层审查和 route/service blast radius 判断 |
| `node scripts/node/verify-repo.js` | 仓库级 full gate | 需要判断“当前改动是否达到仓库级可合入基线” | 不替代 coverage 结论与运行态页面证据 |
| `node scripts/node/verify-coverage.js [frontend|backend|all]` | 覆盖率门禁 | 用户明确要求 coverage、CI 收口或全量质量审计 | 不替代功能正确性 |
| `node scripts/node/verify-ci.js` | CI 总入口 | 需要模拟 CI 最终门禁，或判断“是否可过 CI” | 不替代局部根因定位 |
| `node scripts/node/cli/runtime-gate.js <page-debug args>` | 单路由 / 运行态页面 / 跳转链路证据 | 评估受保护页面、跳转、ready signal、console/runtime 行为 | 不替代静态代码检查与仓库级 full gate |

## Quality Gate Watch Routing

- 仓库管理者、有 GitHub Actions 和 issue 权限：按 `quality-gate-watch.md` 的 GitHub 场景走，用远端 workflow run、artifact JSON 和 issue 状态闭环。
- 无权限贡献者：不要假装能关闭 issue 或确认远端门禁；按 `quality-gate-watch.md` 的本地脚本场景运行 `verify-ci` / `verify-repo` / 定向脚本，并把产物路径、退出码和关键日志作为证据交付。
- 两种场景都不能只凭 issue 正文、PR 绿色标识或口头推测下 QA 结论；需要可复查的运行证据。

## Selection Defaults

- 默认先按最近作用域选门禁，不要一上来就 `verify-ci`。
- Dev Acceptance 只选能证明当前任务结果的最小门禁；完整 lint / build / clippy / workspace test / coverage / hygiene / verify-repo 默认不在本地开发分支自动运行。
- 只改局部前端页面时，先满足 `web/AGENTS.md` 的局部验证，再决定是否升级到仓库级 `test-frontend` 或 `verify-repo`。
- 只改后端局部实现时，先满足 `api/AGENTS.md` 的局部验证，再决定是否升级到 `test-backend` 或 `verify-repo`。
- 命中共享契约、共享 DTO、共享样式场景注册、跨消费者协议时，优先补 `test-contracts`。
- 命中前后端字段兼容 alias 时，必须补 `repo-hygiene` 并在 QA 报告中列出 `@field-contract-compat` warning、废弃计划和测试证据。
- 命中多语言资源、语言切换或 UI 文案抽取时，必须补 `i18n-hygiene`；error 必修，warning 说明删除无效 key 或动态保留的理由。跨 owner 复用只有专项复盘时才加 `--include-cross-owner-warnings`。
- 需要给“仓库级基线是否通过”下结论时，优先 `verify-repo`；需要给“CI 是否可过”下结论时，优先 `verify-ci`。
- 需要讨论覆盖率缺口时，再补 `verify-coverage`；不要拿 coverage 结果替代功能结论。
- 需要运行态页面证据时，优先 `runtime-gate` 或直接 `page-debug`，不要只靠静态阅读代码。
- 证据已经足够支撑当前任务 QA 结论时停止；不要为了显得全面继续叠加无新增覆盖面的门禁。

## Quality Rule Changes

新增、删除或调高/调低质量规则、repo hygiene 规则、AGENTS / skills 门禁时，先把规则本身当成可验收对象：

- 目标：写明要拦截的真实失败模式，不用抽象口号替代。
- 反方样例：至少列出一个不应被拦的合法场景，避免规则过宽。
- 确定性证据：使用 fixture、历史失败、脚本输出或可复现 diff 证明规则能稳定命中目标。
- 预算和停止条件：说明本地执行成本、warning / blocker 归属、何时升级到 PR / Project Health lane。
- 人工确认：规则会改变开发者行为、阻断合并或触碰用户内容时，必须等待用户确认后再进入 enforce。

## Hard Stops

- 没有实际运行证据时，不要声称某条门禁“应该会过”。
- 不能因为局部测试通过，就跳过共享消费者或 blast radius 检查。
- 不能因为 `verify-repo` 通过，就省略用户明确要求的页面运行态、截图或 coverage 证据。
