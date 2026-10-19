# 质量证据与规则所有权

质量门禁保护现有能力、职责边界和数据契约。功能完整性优先于可靠性、性能与可演进性；格式、文件长度和目录压力需要对应职责与行为证据，不能代替调用链判断。

## 证据分层

- `verify.yml` 是 PR / push 合并基线；`quality-gate.yml scope=ci` 是完整项目体检，额外包含完整测试分片、Clippy、一致性、覆盖率和 React Doctor。
- 一次完整体检固定同一个 full SHA。根 `quality-gate-report.json` 必须为 `passed/0`，必需 component 唯一、成功且绑定同 SHA；四个 API coverage 分片经 merge 后才形成覆盖率 component。
- SDK / Host / 内存 EventBus 的单元测试由 runtime-storage-fast 执行，贡献契约与插件事件注册测试由 core-libs 执行；PostgreSQL adapter 与跨层 service 测试由 storage-postgres 分片执行；API lib 与 integration targets 由 API nextest 分片执行。临时专项 workflow 的存在不代表必须重复执行同一组行为。
- 定向 Cargo 命令必须有非零实际通过用例；编译、ignored 和空 filter 不等于行为通过。真实 managed worker 由 CI 先构建，再向行为测试注入路径。
- `state-protocols --skip-live-acp` 不验证真实外部 ACP 服务；静态边界、单元测试与 component 收据也不证明所有页面视觉或生产运行态已经验收。报告分别列明证据范围。

## 静态规则与结构所有权

`check-rust-backend` 管理第一方 Rust 代码，排除 `api/vendor/` 的上游源码和安装产物。第三方漏洞与容器环境警告单列归因；不能借此排除第一方调用路径，也不能用路径包含 vendor 字样代替明确目录边界。测试中的合法失败注入与生产 panic 分别判断。

[repo-hygiene](../../../scripts/node/repo-hygiene/core.js) 对第一方生产代码与测试采用相同结构边界：单文件超过 1500 行、单目录超过 15 个代码文件是 error；1200–1500 行仍为 warning，保留时须说明内聚职责、owner 和验证证据，不能靠截断、压行或挪动无关片段消除压力。计数针对扫描器定义的代码文件，不把全部资源文件数混作目录 finding。

vendor 的规模 finding 仅在精确 `api/vendor/` 边界降为 warning，仍保留 inventory；`api/crates/vendor_named/` 等第一方路径不因此豁免。`api/plugins/installed/` 的安装副本按其可证明的来源排除文件规模检查，普通 plugin 源码继续受检。`warningFiles` 是日志文件数，不是 finding 数，也不能把含 vendor 的报告总数称为第一方问题数。

Rust 文件按领域职责收纳物理子目录时，Cargo package identity、逻辑 module 路径与既有导出保持稳定；不能以目录移动暗改 crate 依赖方向或业务所有权。例如 `domain/src/lib.rs` 将 extension/modeling 类型映射到子目录，`control-plane/src/lib.rs` 将扩展、网络与展示服务映射到职责目录，`storage/durable/postgres/src/lib.rs` 将 repository 映射到子目录，仍通过 `#[path]` 保留原模块入口。窄 Port 定义、业务状态写入与事务 owner 继续遵守 [crate 职责表](../../../api/crates/AGENTS.md)，不因物理拆分转移到调用方或 mapper。拆分测试保留真实场景、断言、mock 注册时序和资源回收边界。

`schema-hygiene` 从正式 migrations 构造结构 inventory。合法表或 constraint 重命名保留原列、键与引用语义；constraint 改名后的删除按新身份处理。无法识别的 DDL 仍报告解析失败，不能把整条 migration 豁免。静态 inventory 不替代 PostgreSQL 的真实迁移执行、约束校验和事务验证。

规则自身需要合法反例与违规反例。删除旧断言的依据是当前 contract 和有效替代证据，不能只因为它失败，也不能反过来让产品兼容过期测试。规模、兼容与翻译 finding 都应保留实际 owner 和处置依据，不能通过扩大路径豁免或清空报告获得健康结论。

## 前端异步与维护告警

第一方 Hook 闭包、重复 React key、废弃组件参数与无效夹具属于可修问题，不能用“上游依赖告警”或“既有债”代替修复。Hook 依赖调整保留 locale、actor、workspace 与 policy 的实际失效语义；不能为了 lint 删除隔离键，也不能加入不稳定的整个 query/session 对象导致重复连接。React Refresh 的组件边界与纯逻辑分别维护，不以虚假导出或规则排除绕过。

行为测试等待真实交互产生的 UI、请求或 store 结果，React 更新纳入 awaited `act` 边界；不能只等待 mock 调用次数或静态占位。cleanup 先卸载并停止组件拥有的订阅、transport、timer 等资源，再恢复 auth、locale、viewport 和全局 mock。原契约要求保持 pending 的流或请求必须显式保留 pending 语义，由卸载/abort 清理，不能为让测试结束伪造成功。夹具使用真实 Router context、唯一资源身份和正确的 ShadowRoot；共享 fixture 不能改变 mock 在产品 import 前注册的顺序。

[warning-capture](../../../scripts/node/testing/warning-capture.js) 只过滤明确的成功工具噪声，不过滤 React `act(...)` 诊断。command stdout/stderr 的运行输出保留；stdout 出现 act 诊断时也写入 warning evidence。即使子进程 exit 0，只要捕获的 stdout 或 stderr 含未等待更新或 Suspense 完成的 act 诊断，wrapper 仍返回失败。不能通过 console 过滤、关闭 act 环境标记或放宽断言代替资源收尾。

组件模块只承担渲染与资源装配。ECharts 的 `lifecycle.tsx` 保留 Boundary 入口，`lifecycle-context.ts` 管理资源 scope；窗口的 Provider / Window 与 controller、overlay context 分开，避免共享 Hook 变更被组件热更新边界吞掉。壳层 assistant 的工具执行与上下文订阅分别归 `assistant-client-tools/factory.ts` 和 `context.ts`，Provider 只组装当前 workspace、locale 和查询缓存。

编辑器的映射校验、排序算法与工具 DTO 草稿留在纯模块；schema field control renderer 保留返回实际输入控件的语义，让 Form.Item 属性注入和通知字段 cloneElement 继续成立。style-boundary 的场景组件与 HTTP 数据夹具分开，scenario manifest 同时追踪两个 owner；行为测试中的夹具提取不改变产品加载、mock 注册或断言的职责。

根前端 `lint` 入口向所有包的 ESLint 透传 `--max-warnings 0`。hygiene 的兼容期限、语义碰撞和结构预警由各自规则与证据结算，不能据 lint 零 warning 宣称这些检查也已通过。

## React Doctor 的原始诊断与源码审查

[Doctor wrapper](../../../scripts/node/react-doctor-gate/core.js) 是门禁。它固定工具 package/version，解析明确的 base/candidate full SHA，并要求 candidate 等于当前 checkout；检查范围是两者之间的 `web/app` 变更，不代表全仓或所有页面。缺少完整范围时拒绝执行，不能用未知 baseline 推导无新增诊断。

`web/app/doctor.config.json` 不配置 rule/file ignore，`ignore.overrides` 保持空。上游命令的 `--fail-on none` 用于收齐含误报在内的原文，再由 wrapper 结算；它不表示无门禁或 advisory。stdout/stderr 原文继续输出，`react-doctor.log` 仅去除 ANSI 控制符，JSON 收据保留工具与 wrapper 各自的 exit code、范围、诊断数量和审查结果。source review 不能从 raw inventory 删除诊断，也不能把 reviewed 数量改称工具零发现。

[diagnostic-review](../../../scripts/node/react-doctor-gate/diagnostic-review.js) 从 `scripts/node/react-doctor-gate/reviews.json` 读取逐项证据。manifest 使用 `schemaVersion`、精确 `toolPackage` 和 `reviews`；每项由 `file + rule + sourceSha256` 绑定当前完整源文件，附明确 `reason` 和非空 `evidence` 仓库路径。重复 file/rule 身份不予授权。审查理由解释具体 contract 或合法反例，引用存在的源码/测试；文件存在不等于该测试已执行，也不能把一个 reviewed rule 扩展为文件级白名单。

源码变化使 digest 失效；同文件新增 rule、工具版本不匹配、缺少理由或有效证据均需重新审查或修复。缺 diagnostics、总数不匹配、工具 error 或无效诊断身份不能形成完整 inventory。wrapper 对工具失败、未解决诊断、不完整报告以及配置中的任意 rule/file/override suppression 返回失败；缺失或无法解析配置也不能作为零 suppression。审查完整且无 suppression 是接受证据的前提，不能通过隐藏诊断使子进程 exit 0 等同门禁通过。

## 持久化与 wire 契约

`@field-contract-compat` 只标记真实的旧输入到当前语义读取，逐项记录 source、alias、remove_by 和测试。未到期不能自动证明应保留，到期也不能直接删读取；退场需要迁移、存量证据和行为回归。以下四处兼容入口保留标记与既有期限：

| 入口 | 保留输入与退出条件 |
| --- | --- |
| `domain/src/mcp_bundle.rs` | MCP v1 顶层 `interface_id` 转当前 `execution_target`，导入校验明确接受 v1；需先提供外部旧 zip / portable-template 的转换与退出方案，不能只看当前 v2 导出。 |
| `orchestration-runtime/src/compiler/node_compilation.rs` | SQL 缺新绑定时从 `config.sql` 编译；需后端统一草稿、版本、发布及模板 normalization，核对持久化 inventory 并完成历史文档迁移回归。 |
| `agent-flow/schema/agent-flow-field-renderers.tsx` | 旧 `config.sql` 可显示，编辑后写 `bindings.sql`；编译器仅在内存补绑定不等于保存格式迁移，退场需与后端同步。 |
| `storage/durable/postgres/src/orchestration_runtime_repository/client_trajectory/semantic.rs` | 从旧 event 的 `fact.value` 恢复 section；维护迁移会保留含 NUL 的 `semantic_legacy_retained` 原文，旧格式 importer trigger 也仍能写 legacy 行。先无损转换 retained 数据、关闭或转换旧写入并验证全 scope inventory，再移除 reader。 |

MCP v1 serde/上传 zip、SQL 编译与旧文本编辑、轨迹迁移后的原文/cursor/anchor 读取各有真实测试；它们证明所测边界，不证明用户持有旧数据的数量或部署迁移已经完成。历史 SQL 格式退场约束见 [请求架构与调用生命周期](../interface-lifecycle.md)。

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

## 写入与读取职责

Trace projection 的写入对象由 `control-plane-contracts` 管理。共享 alias 使用 typed object 和 `Arc<Value>`，成员变更按 copy-on-write 隔离；标量和数组可作为普通 JSON 保留，但向其插入对象字段必须显式返回错误并保持原值。业务端传播错误，持久化边界才展开共享 alias，读取 DTO 与 wire JSON 形状保持不变。

选择性备份中的 PostgreSQL bytea 由存储 adapter 解码，使用既有 `base16ct`，保留大小写十六进制与空字节串并拒绝无效内容；业务层不重复实现编码算法。

## 执行位置

项目体检优先在 GitHub Actions 执行，持续失败的剩余门禁不超过三项时才考虑本地取证。warning、coverage 与执行日志统一放在 `tmp/test-governance/`。完整通过只结算已有证据覆盖的目标，不能据此宣称完成独立安全审计、全产品人工验收或尚未执行的迁移与回归。

项目体检和手动运行默认使用 React Doctor `--full`，强制覆盖全应用源文件，配置中的 diff 范围不能缩小扫描。PR 默认 diff，可用 `REACT_DOCTOR_SCAN_MODE=full|diff` 显式指定；收据分别保留请求/实际 scan mode 与 candidate/base，实际 mode 不匹配立即失败。changedFiles 只记录变更元数据，不冒称 full 扫描清单；全量诊断需逐项结算。

全量诊断涵盖 `web/app/src`、构建/Vite owner 和 `package.json` 的依赖图；这些路径同样逐项审查。大型审查数据采用 schemaVersion 2：入口 `reviewFiles` 明确列出按 owner 收纳的 manifest，每项仍绑定完整源文件和证据摘要；缺失、重复、越界分组或工具版本不匹配不能部分接受。schemaVersion 1 保留为单文件读取契约。
