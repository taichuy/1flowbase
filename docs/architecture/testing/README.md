# 质量证据与规则所有权

质量门禁保护现有能力、职责边界和数据契约。功能完整性优先于可靠性、性能与可演进性；格式、文件长度和目录压力需要对应职责与行为证据，不能代替调用链判断。

## 证据分层

- `verify.yml` 是 PR / push 合并基线；`quality-gate.yml scope=ci` 是完整项目体检，额外包含完整测试分片、Clippy、覆盖率、特殊 feature 场景和 React Doctor。
- 一次完整体检固定同一个 full SHA。根 `quality-gate-report.json` 必须为 `passed/0`，必需 component 唯一、成功且绑定同 SHA；四个 API coverage 分片经 merge 后才形成覆盖率 component。
- SDK / 内存 EventBus 的单元测试由 runtime-storage-fast 执行，贡献契约与插件事件注册测试由 core-libs 执行；Host、orchestration-runtime 和 PostgreSQL adapter 的完整包测试由各自 coverage owner 执行，跨层 control-plane-postgres-tests 仍由 storage-postgres 四分片完整执行，第一分片另执行 model-usage-report-postgres Node 集成；API lib 与 integration targets 由 API coverage nextest 分片执行。临时专项 workflow 的存在不代表必须重复执行同一组行为。
- 定向 Cargo 命令必须有非零实际通过用例；编译、ignored 和空 filter 不等于行为通过。真实 managed worker 由 CI 先构建，再向行为测试注入路径。
- `state-protocols --skip-live-acp` 不验证真实外部 ACP 服务；静态边界、单元测试与 component 收据也不证明所有页面视觉或生产运行态已经验收。报告分别列明证据范围。

## 门禁执行与结果复用

[English](gate-execution.en.md)

- 完整体检的 `clippy --all-targets` 同时承担对应包的类型检查，CI matrix 与默认汇总不再重复要求相同配置的三个 `cargo check` 分片。PR 基线没有 Clippy，继续保留 check；定向 check 命令仍可独立使用。feature 专项和行为测试不由 Clippy 替代。
- API 普通四分片与 coverage 四分片的 package、默认 features、nextest partition 和实际 test identity 并集相同，完整 CI 由 coverage 一次承担行为与覆盖率验收。控制平面、orchestration-runtime、runtime-extension-host 和 storage-durable-postgres 的完整包行为测试同样交给各自 coverage owner；前三者另保留显式 `cargo test --doc`，包括 Host compile-fail 用例和未来文档示例。仅 `QUALITY_GATE_SCOPE=ci` 的普通 runtime/storage 分片省略这三个重复包；独立手动 / PR shard、`test-backend` 与 `verify-backend all` 保留原测试集合。control-plane-postgres-tests 的四分片与第一分片的 SQL 报表集成继续运行。
- API coverage 分片成功执行非零用例后才写回执；merge 核对同 SHA、package/partition、实际通过数、inventory 与非空 profile 数量，再验证完整、不重叠的可执行用例并集。ignored 单独按全量集合计数，不能把 nextest 的 partition/filter skipped 当作 ignored，也不能用旧产物补缺失分片。
- `verify/backend.js` 是后端 Cargo 命令和受保护场景执行校验的 owner；无参数 `test-backend` 与 `verify-backend all` 执行静态检查和一次 workspace 测试，图像/多模态场景已由该集合覆盖。独立 `image-llm-vision` 专项仍可运行。每条受保护测试仍须实际执行非零用例，邻接成功、全部 ignored、编译成功和缺失日志不能填补证据。
- `tooling data-contracts` 使用同一份 migration inventory 和源码分析结果，生成原有 schema、raw JSONB、query-contract、capacity JSON/Markdown 报告。schema error 和 query-contract failure 继续阻断；raw JSONB、增长规划和未测量容量保留原来的 advisory / unverified 语义。查询契约等必需配置缺失或损坏时继续失败。独立报告入口仍服务于定向诊断。
- 专项测试与全量测试只有在用例及运行条件等价时才能复用结果；nextest/libtest、线程栈、feature 和 coverage instrumentation 的差异需要单独判断。已核对实际用例 identity 的 consistency-control-runtime/storage/api、official-i18n-seed、image-llm-vision 默认 feature 场景由普通 / coverage 包测试完整执行，完整 CI 不再重跑这五个专项 owner，必需 component 从 29 降为 24。专项手动入口及 PR 的 seed / image-llm 门禁保持有效；history-maintenance 的特殊 feature 和独有场景继续单独执行。四基座、coverage 阈值和缺失 / 失败 / 错误 SHA receipt 的拒绝条件不变。
- 五个前端 consumer contract 用例由前端验收拥有：`frontend-pr` 显式运行 contracts；仓库级 `frontend/all` 通过常规测试与 page-regression 的并集覆盖全部用例，tooling 不再重复执行。`test-frontend full` 单独不包含全部页面回归，不能作为该并集的替代。

开发定向入口直接把选择条件交给原生 runner：`test-backend.js --package <crate> [--lib | --test <target> | --bin <target>] [--filter <name>] [--exact]`，以及 `test-frontend.js fast [--] <file> [-t <pattern>]`。后端选择须带 package；不支持、缺值或冲突参数在启动前报错，不能静默扩大成全量测试。前端显式文件不继承快速套件排除名单，未指定文件仍保留该范围；定向执行通过独占的原生 Vitest JSON 报告核验非零通过，不能把全部 skipped 当成功。资源预算继续生效；完整 CI 在既有 owner 中实际运行一条 Rust 和一条前端定向命令。

协议专项只拥有官方供应商测试、插件打包和真实 wire/runtime 契约。仓库 Rust 行为测试交给上述普通/coverage owner，Node 工具测试交给 `repo-tooling`，不在协议任务重复编译执行。完整 CI 仍必须同时取得这些 owner 的候选收据；单独协议绿灯不代表全仓验收。供应商独有测试、非零执行校验、真实协议错误/传输 oracle 和源码/产物配对保持有效。

API coverage 使用 [LLVM 原生 `%Nm` 合并池](https://clang.llvm.org/docs/SourceBasedCodeCoverage.html)：N 来自当前测试并行度，每个二进制由运行时加锁合并；不再用 `%p` 为每个测试进程留下完整 profile。分片集合、执行计数、SHA 与覆盖率阈值不变。首分片先在同一真实插桩二进制上比较逐进程与池化的完整 coverage counters，并以丢失一次执行作为反例；证明失败则该分片不能产生成功收据。等价证据随首分片 artifact 保留，最终仍合并全部 API 测试的 profiles。

`cargo llvm-cov report` 会扫描 target 中的可执行对象，包括旧缓存。分片和 merge 在建立当前 inventory 前使用原生 `cargo clean --package api-server` 重建被测包，避免旧对象把已删除源码或旧实例化计入分母；其它依赖缓存及已构建的 SDK worker 保留。此做法沿用 [cargo-llvm-cov 的测量包清理机制](https://github.com/taiki-e/cargo-llvm-cov/blob/v0.9.1/src/clean.rs)，不通过源码排除提高覆盖率。merge 拒绝当前不存在的 API-owned source，并保留 LLVM 实际对象列表日志；线上受控缓存反例验证污染被发现、清理后当前计数与干净基准一致。

统一治理验收需要冻结候选的全量 Rust 测试和完整 CI：记录通过、失败、忽略/跳过数及核心场景清单，分别量化编译、测试、CI 墙钟时间和累计 runner 时间。根收据须满足前述 candidate、component 与成功条件；局部脚本通过不能结算整体任务。合并重复执行允许执行次数减少，但核心场景不得缺失。历史时间只作注明源码、环境、缓存差异的参考；未测量值保留未知。

本地后端预算使用 `min(CPU 容量, max(4, floor(CPU 容量 / 2)))` 和 `min(有效内存容量, max(8 GiB, 有效内存容量 / 2))`。CPU 和内存配额由 `dev-rust.slice` 聚合，父组和前端默认不限。GitHub Actions 的验证与预热使用全部 runner CPU，不施加本地内存配额。具体安装与系统限制由 [scripts/README](../../../scripts/README.md) 维护，不扩展为跨前端、数据库和其他会话的实时准入系统。

迁移拒绝场景使用同一正式迁移集，并让预期失败的迁移独占连接、返回错误后显式关闭，避免失败留下的 session advisory lock 等待连接池淘汰。正常迁移仍走生产入口；保留原子回滚、账务、历史 checksum 等断言，不通过关闭生产迁移锁或删除失败场景加速。

API 通用 fixture 通过 `PostgresTestSchema::create()` 为每个测试创建独立空数据库和 schema，显式设置 startup search_path，再执行生产 Migrator 和完整业务 bootstrap。文件目录同样逐 fixture 隔离；数据库 guard 只回收自己拥有的临时库。历史迁移与恢复 fixture 保留各自原有入口。

## 静态规则与结构所有权

`check-rust-backend` 管理第一方 Rust 代码，排除 `api/vendor/` 的上游源码和安装产物。第三方漏洞与容器环境警告单列归因；不能借此排除第一方调用路径，也不能用路径包含 vendor 字样代替明确目录边界。测试中的合法失败注入与生产 panic 分别判断。

[repo-hygiene](../../../scripts/node/repo-hygiene/core.js) 对第一方生产代码与测试采用相同结构边界：单文件超过 1500 行、单目录超过 15 个代码文件是 error；1200–1500 行仍为 warning，保留时须说明内聚职责、owner 和验证证据，不能靠截断、压行或挪动无关片段消除压力。计数针对扫描器定义的代码文件，不把全部资源文件数混作目录 finding。

vendor 的规模 finding 仅在精确 `api/vendor/` 边界降为 warning，仍保留 inventory；`api/crates/vendor_named/` 等第一方路径不因此豁免。`api/plugins/installed/` 的安装副本按其可证明的来源排除文件规模检查，普通 plugin 源码继续受检。`warningFiles` 是日志文件数，不是 finding 数，也不能把含 vendor 的报告总数称为第一方问题数。

Rust 文件按领域职责收纳物理子目录时，Cargo package identity、逻辑 module 路径与既有导出保持稳定；不能以目录移动暗改 crate 依赖方向或业务所有权。例如 `domain/src/lib.rs` 将 extension/modeling 类型映射到子目录，`control-plane/src/lib.rs` 将扩展、网络与展示服务映射到职责目录，`storage/durable/postgres/src/lib.rs` 将 repository 映射到子目录，仍通过 `#[path]` 保留原模块入口。窄 Port 定义、业务状态写入与事务 owner 继续遵守 [crate 职责表](../../../api/crates/AGENTS.md)，不因物理拆分转移到调用方或 mapper。拆分测试保留真实场景、断言、mock 注册时序和资源回收边界。

`schema-hygiene` 从正式 migrations 构造结构 inventory。合法表或 constraint 重命名保留原列、键与引用语义；constraint 改名后的删除按新身份处理。无法识别的 DDL 仍报告解析失败，不能把整条 migration 豁免。静态 inventory 不替代 PostgreSQL 的真实迁移执行、约束校验和事务验证。 有限 owner 契约在 `schema-hygiene/ownership-config.json` 集中声明，由原 `loadConfig` 入口合并：mixed system/workspace contribution authority 保留 installation/workspace 复合身份、级联引用、scope lookup index 与 validation/cleanup trigger；protocol capsule 保留 flow-run/kind/slot 身份、run cascade 和 hard-expiry index。Trigger inventory 按 migration 顺序跟踪 create/drop 及调用形状，未知变形报告解析错误；它不验证 PL/pgSQL 函数体、加密、expiry 或 replay 的运行语义。

规则自身需要合法反例与违规反例。删除旧断言的依据是当前 contract 和有效替代证据，不能只因为它失败，也不能反过来让产品兼容过期测试。规模、兼容与翻译 finding 都应保留实际 owner 和处置依据，不能通过扩大路径豁免或清空报告获得健康结论。

前端测试生命周期规则由 [`testing-library-lifecycle-rules`](../../../scripts/node/testing-library-lifecycle-rules/index.js) 在原 ESLint namespace 下精化，上游 metadata、options、listeners 和其它诊断保留：直接 awaited async `act` 只有在真实 RTL `render/renderHook/fireEvent` binding 且不混入 `waitFor/findBy/user-event` 时接受；同步或未 await 的 scope、空 callback 与已异步包装的等待仍报告。手动 `cleanup` 只在其所有引用均归属可证明的 `afterEach`，并在同一顺序块（含 `finally` 或已 await 的 act）先卸载、再调用真实 auth/design store reset、QueryClient.clear 或应用 i18n.changeLanguage 时接受；独立 cleanup、mock restore、未知或条件 reset 不构成例外。renderer registry 只按已知 schema registry 工厂 import contract 区别于 RTL render；HMR 混合导出只接受 `_tests` 内直接导入 Vitest/RTL 的 unit harness，browser fixture 和产品模块不豁免。React 19 [Effect Event contract](https://react.dev/reference/react/useEffectEvent) 要求不纳入依赖且仅由 Effect 调用，并不保证函数 identity 稳定；针对旧 exhaustive-deps 的适配须证明真实 React binding、所有引用为 effect 内调用，且上游 missing 报告的建议仅增加这些 Event 名、不移除或改变其它依赖。上游 message/suggestion AST 文本契约不匹配、混合普通缺失项、普通 handler 或未知 binding 均保留报告。受控 Node 正反例随 `test-scripts` 自动发现；它们与线上真实 fixture 生命周期日志分别结算，不以 lint 通过代替异步行为取证。

## 前端异步与维护告警

第一方 Hook 闭包、重复 React key、废弃组件参数与无效夹具属于可修问题，不能用“上游依赖告警”或“既有债”代替修复。Hook 依赖调整保留 locale、actor、workspace 与 policy 的实际失效语义；不能为了 lint 删除隔离键，也不能加入不稳定的整个 query/session 对象导致重复连接。React Refresh 的组件边界与纯逻辑分别维护，不以虚假导出或规则排除绕过。

行为测试等待真实交互产生的 UI、请求或 store 结果，React 更新纳入 awaited `act` 边界；不能只等待 mock 调用次数或静态占位。cleanup 先卸载并停止组件拥有的订阅、transport、timer 等资源，再恢复 auth、locale、viewport 和全局 mock。原契约要求保持 pending 的流或请求必须显式保留 pending 语义，由卸载/abort 清理，不能为让测试结束伪造成功。夹具使用真实 Router context、唯一资源身份和正确的 ShadowRoot；共享 fixture 不能改变 mock 在产品 import 前注册的顺序。

[warning-capture](../../../scripts/node/testing/warning-capture.js) 只过滤明确的成功工具噪声，不过滤 React `act(...)` 诊断。command stdout/stderr 的运行输出保留；stdout 出现 act 诊断时也写入 warning evidence。即使子进程 exit 0，只要捕获的 stdout 或 stderr 含未等待更新或 Suspense 完成的 act 诊断，wrapper 仍返回失败。不能通过 console 过滤、关闭 act 环境标记或放宽断言代替资源收尾。

组件模块只承担渲染与资源装配。ECharts 的 `lifecycle.tsx` 保留 Boundary 入口，`lifecycle-context.ts` 管理资源 scope；窗口的 Provider / Window 与 controller、overlay context 分开，避免共享 Hook 变更被组件热更新边界吞掉。壳层 assistant 的工具执行与上下文订阅分别归 `assistant-client-tools/factory.ts` 和 `context.ts`，Provider 只组装当前 workspace、locale 和查询缓存。

编辑器的映射校验、排序算法与工具 DTO 草稿留在纯模块；schema field control renderer 保留返回实际输入控件的语义，让 Form.Item 属性注入和通知字段 cloneElement 继续成立。style-boundary 的场景组件与 HTTP 数据夹具分开，scenario manifest 同时追踪两个 owner；行为测试中的夹具提取不改变产品加载、mock 注册或断言的职责。

根前端 `lint` 入口向所有包的 ESLint 透传 `--max-warnings 0`。hygiene 的兼容期限、语义碰撞和结构预警由各自规则与证据结算，不能据 lint 零 warning 宣称这些检查也已通过。

`@rc-component/util@1.12.0` 的 `useDelayState` 补丁回移已合并的 [upstream #806](https://github.com/react-component/util/pull/806)：卸载复用既有 `cancelPending`，同时取消毫秒与 RAF 更新，保持替换、正常提交和实例隔离语义。ESM/CJS 的真实已安装产物由 `test/_tests/dependency-lifecycle` 覆盖；升级到包含该修复的版本时删除这两处补丁并保留行为回归。测试不得通过延时、全局清空 timer 或替换真实 Hook 掩盖卸载泄漏。

## React Doctor 的原始诊断与源码审查

[Doctor wrapper](../../../scripts/node/react-doctor-gate/core.js) 是门禁。它固定工具 package/version，解析明确的 base/candidate full SHA，并要求 candidate 等于当前 checkout；检查范围是两者之间的 `web/app` 变更，不代表全仓或所有页面。缺少完整范围时拒绝执行，不能用未知 baseline 推导无新增诊断。

`web/app/doctor.config.json` 不配置 rule/file ignore，`ignore.overrides` 保持空。上游命令的 `--fail-on none` 用于收齐含误报在内的原文，再由 wrapper 结算；它不表示无门禁或 advisory。stdout/stderr 原文继续输出，`react-doctor.log` 仅去除 ANSI 控制符，JSON 收据保留工具与 wrapper 各自的 exit code、范围、诊断数量和审查结果。source review 不能从 raw inventory 删除诊断，也不能把 reviewed 数量改称工具零发现。

[diagnostic-review](../../../scripts/node/react-doctor-gate/diagnostic-review.js) 从 `scripts/node/react-doctor-gate/reviews.json` 读取逐项证据。manifest 使用 `schemaVersion`、精确 `toolPackage` 和 `reviews`；每项由 `file + rule + sourceSha256` 绑定当前完整源文件，附明确 `reason` 和非空 `evidence` 仓库路径。重复 file/rule 身份不予授权。审查理由解释具体 contract 或合法反例，引用存在的源码/测试；文件存在不等于该测试已执行，也不能把一个 reviewed rule 扩展为文件级白名单。

源码变化使 digest 失效；同文件新增 rule、工具版本不匹配、缺少理由或有效证据均需重新审查或修复。缺 diagnostics、总数不匹配、工具 error 或无效诊断身份不能形成完整 inventory。wrapper 对工具失败、未解决诊断、不完整报告以及配置中的任意 rule/file/override suppression 返回失败；缺失或无法解析配置也不能作为零 suppression。审查完整且无 suppression 是接受证据的前提，不能通过隐藏诊断使子进程 exit 0 等同门禁通过。

## 持久化与 wire 契约

本次三类 0.5 前格式已经明确退出支持。MCP bundle 仅接受 `1flowbase.mcp.bundle/v2`，Tool 必须使用 `execution_target`，顶层 `interface_id` 拒绝反序列化；SQL 节点只使用 `bindings.sql`，编译器对仍含 `config.sql` 的文档明确报错，编辑器不读取旧字段。

旧 client trajectory semantic event/section 不再由当前读取器恢复，旧 semantic-history mover 已删除。forward migration `20261019210000_retire_pre05_client_semantics.sql` 关闭旧 event 投影入口，并令旧 step/section 读取明确失败；原有历史行保留，不静默丢弃或改写。当前 content/timing、NUL 原文、cursor/high-water 与目录压缩继续由现有 owner 维护。历史 archive codec 迁移测试固定到截止前的正式 migration prefix；它们验证不可变历史迁移，不代表当前版本还接受旧 importer。回归分别覆盖当前格式成功与旧格式拒绝，不保留“旧输入仍可用”的过期断言。

此决定只适用于上述 MCP、SQL 与 client semantic 三类入口；不据此删除其他版本化 codec、当前协议字段或原始归档功能。测试数据库使用隔离 schema；本任务未对实际部署数据库执行迁移或删除数据。

`CompiledLlmRuntime` 的 `provider_instance_id / provider_instance_display_name / protocol / routing` 是当前 wire 字段，不是临时 serde alias。main 计划允许省略并用 default，实际身份在运行时解析；failover 计划仍冻结目标身份、显示名、协议与 queue policies。四处注释描述这一契约，不附虚构删除期限；字段与 serde 行为保留。[serde 边界测试](../../../api/crates/orchestration-runtime/src/_tests/compiled_plan_compatibility_tests.rs) 覆盖非空 failover、main 省略格式、旧缺 display name 及错误类型/必需字段拒绝，不能替代完整计划持久化恢复验收。

## 翻译资源所有权

配置驱动翻译仍需明确 namespace/key 的静态所有权。`BlockStudioWorkspace.tsx` 的 interfaces、block-tree、configuration 三个 section 使用静态 `i18nText('frontstage', key)` 回调，保持渲染时翻译；`NetworkEgressPoolsPanel.tsx` 的四个已知 health 值使用明确 key map，未知 health 保留既有 fallback。扫描器无需为任意动态字符串扩大白名单。

壳层没有独立 `auto.subsystem` 资源；对应导航 fixture 使用完整 `embeddedApps:auto.subsystem`，与真实 embedded-apps 页面复用同一双语资源。builtin 与 HostExtension 的实际生产声明是消费者证据，fixture 或允许任意 `label_key` 的结构本身不能证明存在外部使用。

以下五组仅在单 locale 同值，完整翻译向量和真实消费者不同，保留独立 key 与原可见文案，并继续由 `i18n-hygiene` 报告碰撞：

| Owner / keys | 不同语义与翻译证据 |
| --- | --- |
| frontstage `auto.operation / files.actions` | Studio 插入操作栏与文件记录动作栏；en `Operation / Actions`。 |
| frontstage `auto.save / files.save` | Studio source 保存与文件编辑表单提交；en `save / Save`。 |
| frontstage `design.tab_route_segment / files.url` | `/tabs/` 下路由片段与文件 URL；en `Route segment / Access path`。 |
| notifications `recipient_count / system_notification_recipients` | 日志人数与 user_ids 选择；zh `收件人数 / 收件人`。 |
| ui-management `default / set_default` | 默认状态 Tag 与设置默认命令；en `Default / Set default`。 |

不存在消费者的资源可删除；不同语义不能为消除同值 warning 强制合 key 或改写可见翻译。未知动态引用仍需真实 producer 与 renderer owner，不因上述有限证据自动保留所有未引用资源。

审查证据也绑定文件内容 SHA-256；组件源码未变但契约或行为夹具改变时，原审查失效，不能仅凭路径仍存在接受旧结论。

每条 review 只依赖该规则实际涉及的不变量、直接 consumer 和相关测试，不把整个 router、设置页 shell 或整片功能测试默认挂到所有组件。目标完整源文件的摘要始终保留，移除无关证据前须核对原理由与实际调用；证据收窄不改变原始诊断或新增 rule 的审核要求。`SignInPage` 的 `filing_html` 明确信任具有 `system.branding.save` 权限的站点 HTML 发布者，原样展示是既有功能；review 同时绑定后端写权限与前端原文展示证据，不扩展为任意不可信 HTML 的豁免。

## 写入与读取职责

Trace projection 的写入对象由 `control-plane-contracts` 管理。共享 alias 使用 typed object 和 `Arc<Value>`，成员变更按 copy-on-write 隔离；标量和数组可作为普通 JSON 保留，但向其插入对象字段必须显式返回错误并保持原值。业务端传播错误，持久化边界才展开共享 alias，读取 DTO 与 wire JSON 形状保持不变。

选择性备份中的 PostgreSQL bytea 由存储 adapter 解码，使用既有 `base16ct`，保留大小写十六进制与空字节串并拒绝无效内容；业务层不重复实现编码算法。

## 执行位置

项目体检优先在 GitHub Actions 执行，持续失败的剩余门禁不超过三项时才考虑本地取证。warning、coverage 与执行日志统一放在 `tmp/test-governance/`。完整通过只结算已有证据覆盖的目标，不能据此宣称完成独立安全审计、全产品人工验收或尚未执行的迁移与回归。

项目体检和手动运行默认使用 React Doctor `--full`，强制覆盖全应用源文件，配置中的 diff 范围不能缩小扫描。PR 默认 diff，可用 `REACT_DOCTOR_SCAN_MODE=full|diff` 显式指定；收据分别保留请求/实际 scan mode 与 candidate/base，实际 mode 不匹配立即失败。changedFiles 只记录变更元数据，不冒称 full 扫描清单；全量诊断需逐项结算。

全量诊断涵盖 `web/app/src`、构建/Vite owner 和 `package.json` 的依赖图；这些路径同样逐项审查。大型审查数据采用 schemaVersion 2：入口 `reviewFiles` 明确列出按 owner 收纳的 manifest，每项仍绑定完整源文件和证据摘要；缺失、重复、越界分组或工具版本不匹配不能部分接受。schemaVersion 1 保留为单文件读取契约。
