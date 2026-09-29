# Quality Gate Health Review

## Scope And Evidence

目标是恢复项目自身质量检测与验收能力，更新改造后失效的规则和 fixture，修复有效门禁发现的产品回归。第三方依赖、容器镜像和环境漏洞 / warning 仅记录，不升级、不屏蔽检测。本报告不是全项目无缺陷声明。

- 历史窗口：2026-08-29 至 2026-09-29，冻结基线 `72e0e82610b9c16365eda6b32005d24c43023e84`，899 个非 merge 提交。
- 起点对照：`b1daf67728d1a9e17fcbfcff7b01d1c8296b948c`。
- [基线完整线上运行](https://github.com/taichuy/1flowbase/actions/runs/36556951217)。失败日志和扫描报告下载到 `tmp/test-governance/health/`。
- [Root #2163](https://github.com/taichuy/1flowbase/issues/2163) 保存冻结候选、Test Batch、最终 run / artifact 和未闭合项；[检测修复 #2164](https://github.com/taichuy/1flowbase/issues/2164)、[完整验收 #2165](https://github.com/taichuy/1flowbase/issues/2165) 承载交付。
- 开发期间未在本地运行门禁、Cargo 编译、行为测试或浏览器。语法、定向格式和 diff 检查不等于行为验收；候选结果以 Root 的线上 receipt 为准。
- 读取用户记忆及相关旧测试 / 门禁边界反馈；历史接口生命周期记忆只作为入口，当前 AGENTS、源码和运行证据优先。

## History And Rule Coverage

频次来自该窗口 `git log --no-merges --name-only`，表示文件被提交触达的次数，不能直接推导为代码质量差。

| 热点 owner | 次数 | 规则与实际验收 owner | 本次处理 |
| --- | ---: | --- | --- |
| `api-server/extension_bus/interface_contributions.rs` | 125 | `api/AGENTS.md`、`crates/AGENTS.md`、interface lifecycle / 四基座 receipt | 保留真实装配与绑定门禁；tooling green 不能替代基座验收 |
| `routes/application_public_api/openai.rs` | 39 | Protocol → Canonical Interface → typed Handler；sealed identity / continuation / stream completion | 补齐 AI owner 路由；修正旧 session 与 transport oracle |
| `api-server/src/lib.rs` | 30 | 唯一 Backend composition root，外部入口装配、认证、收尾 | 用现有 foundation 与 API 行为矩阵，不新建并行 registry |
| `provider_runtime/transport_session_lifecycle.rs` | 29 | provider Host owner、会话生命周期、取消与终态分离 | WS mock 使用真实 upstream transport 与 terminal barrier |
| `orchestration_runtime_repository/mod.rs` | 28 | `storage/AGENTS.md`：adapter/SQL/mapper；正式 migrations | 更新物理 schema 检测和过时 payload fixture |
| `control-plane/.../provider_invoker.rs` | 28 | Business/Execution owner、完整事实与 live delta 分离 | 保留完整语义持久化；旧 token delta 持久化断言更新 |
| `routes/application_public_api/native.rs` | 27 | native / compatibility 协议入口与错误原样 contract | 加入 AI / state-protocols 路由，不用泛化 sanitization 代替上游错误 |
| `application_runtime/interface_runtime_reads.rs` | 26 | 后端唯一真值、lazy detail、不以查询产生写副作用 | 复用真实 HTTP lazy/read-side-effect 回归 |
| `.github/workflows/quality-gate.yml` | 11 | QA gate lane、candidate-bound evidence、必需 job outcome | 一次解析冻结 SHA，拒绝跨候选报告与失败 coverage merge |

## QA Skill And AGENTS Alignment

| 真值 / 规则 | 审计发现 | 修复或证据边界 |
| --- | --- | --- |
| Root AGENTS：1500 行、目录 15 文件、AGENTS 200 行 | `repo-hygiene` 未扫描 `.agents`、CJS/ESM，且缺 AGENTS 长度提示 | 纳入实际维护源码，200 行以上 advisory；私有 `.memory`、历史计划与生成产物保持排除 |
| Root AGENTS：未引用 i18n key 必须 warning，保留动态引用需说明 | scanner 只读取旧 bootstrap，未跟上 08-30 的 application resources split | 读取 bootstrap 直接导入的本地资源模块；跨 owner 正例和真实 unused 负例，未删除文案/key |
| Root AGENTS：字段兼容标记必须 warning | scanner 自己的 regex 也被识别成兼容注释 | 仅从实际注释识别；inline/block/comment 正例、regex/string 负例 |
| `web/AGENTS.md` / frontend gate | 有效 `missing-optimize-dep` 发现 lazy 使用的 `lru-cache` 配置遗漏 | 补 Vite optimizeDeps；不调整 UI 文案、布局、依赖版本 |
| `api/AGENTS.md`：生产请求不得引入 panic/unwrap、Host-owned 网络边界 | Rust cfg(test) 扫描可能吞掉后续生产源码 | 词法 masking 保持行号，test 字段/use/单行项与 production cfg 正反例 |
| `crates/AGENTS.md` / `storage/AGENTS.md` | QA reference 仍引用旧 `storage-postgres` owner，workspace 全量被误写成最小证据 | 更新为 `storage/durable/postgres` / `storage-durable-postgres`，按实际构建成本与风险选证据 |
| `control-plane-postgres-tests/AGENTS.md`：隔离 schema、正式迁移、保留业务断言 | 两个历史迁移 fixture 在最新 schema 重放已移除列的 SQL | 在对应正式 migration 前缀 seed，再执行原 SQL、幂等重放、最终升级；历史 migration blob 未改 |
| QA：源码/编译/行为/运行态分别结算 | 聚合只看 status/exitCode，错 SHA 和 coverage merge 失败可能漏拦 | 严格 receipt commit、必需 job 结果；warning-only 正例及缺失/错 SHA/cancelled/skipped merge 负例 |
| QA：沿用已有授权，产品语义改变才重新定界 | 引用文件曾要求重授权或默认本地重复全量 | 更新授权与资源口径；本次在线策略是任务覆盖，不写成所有任务永久规则 |
| QA：上游错误 shape 以当前 contract 为准 | 通用“消毒上游错误”与当前 Gateway passthrough 契约冲突 | 按协议与 owner 核对原样上游错误，不把真实错误 body 当第三方漏洞 warning |

## Obsolete Rules, Fixtures And Real Defects

没有证据支持删除整个现存门禁。有效门禁发现了真实问题；删除 / 更新的是失效的检测范围、过时预期和无消费者遗留文件。

| 类别 | 源码 / 历史证据 | 处理与识错能力 |
| --- | --- | --- |
| 无效遗留文件 | QA `task-mode-checklist.md.orig` 无消费者 | 删除；保留 canonical checklist |
| 漏掉现存脚本测试 | test discovery 缺 `.test.cjs/.test.mjs` | 新格式纳入，保持 `_tests/*.js`；不把长验收 `run.cjs` 当 unit test |
| schema 契约过时 | `76b074859` 已拆 payload、archive、snapshot、canonical 表 | 精确检查列/类型/nullability、PK、FK/delete action、unique、CHECK、index；默认 managed-table 不全局豁免 |
| 真实代码 / 配置问题 | 基线 fmt / Clippy 与 Vite 失败 | 小范围格式、借用、常量错误与 fixture type alias 修正；无产品语义变化 |
| 旧 delta 断言 | `76b074859`：live delta ephemeral，完整语义才 durable | writer 与 preview 测试验证文本/顺序/完整输出；禁止恢复旧碎片持久化来收绿 |
| 旧历史归属断言 | `897383a56`：删除用户保留历史资源 | backup preflight/restore 保留原 UUID、姓名与用户已删除状态；真正父资源缺失负例已有覆盖 |
| 真实归档回归 | archive query 仍读 `node_runs.error_payload`，restore 仍向已移除 payload 列写入 | 查询 `node_run_details` 并恢复 lossless original，写正式 `node_run_records`；原 HTTP export/import 与错误矩阵继续验收 |
| 旧测试 SQL | API fixture 直接读写 `node_runs` payload | 使用正式 operational view；metadata-only 查询不扩大 hydration；保留原业务断言 |
| paired-source 过期 | 官方 `f7d8214` 已修正 DeepSeek `request_prepared` fixture / SDK | pin 已合入修正版本；不改第三方或外部仓源码、不取消 provider test |
| 旧 session oracle | `ab3e5c8f2` 已封印 application/key/workspace/session/thread 身份 | direct fixture 精确模拟当前 seal，缺 identity、错误 seal、residual 丢失仍拒绝；不忽略 session-id |
| 旧 WS transport / mock | upstream 已实际选择 Responses WS，旧 oracle 只认 SSE；WS 缺 barrier | 按明确 transport 核对 nonce/model/run/arrival；两种 transport 均须首 delta 后持有终态 |
| interruption fixture 未证明 commit 边界 | 同 tick 写 SSE 后 destroy，Gateway 未观察到 delta就可合法恢复 | 首个 nonce-bearing delta 经真实 parser 观察后才释放断流；不能把 one arrival 断言直接放宽为 two |
| 旧 MCP 时序 | `d485f4` 旧提前续接；`5d5c786c` 当前在 host 完成后保存 continuation/history | 同 response completed 后续接；未完成/失败/不相关 terminal 不能放行，审批 id/上游 id/owner 义务不变 |

曾怀疑 `write_node_run_record` 的 `node_run_id` 引用无效；完整迁移核对发现 `20260529123000` 已添加 `node_run_id generated as(id)`，且基线节点详情测试通过。因此撤回该候选，不改历史迁移、不新增修复迁移。这说明源码片段必须与完整迁移历史交叉核对。

## Acceptance Matrix And Limits

| 维度 | 线上证据入口 | 可结算范围 / 限制 |
| --- | --- | --- |
| 工程与检测可信度 | repo-tooling、gate scanner 正反 fixture、aggregate tests | 扫描范围与识错能力；源码 regex 不证明所有 runtime 行为 |
| 前端契约与配置 | repo-frontend、React Doctor、frontend coverage | 类型、消费者、结构规则与测试；手动 React Doctor 显式使用 protected baseline `72e0e8261`，它检查本候选 diff，月度源码债另列 |
| 后端行为 / 数据 / 错误 | 四 API shards、PG shards、control-plane、storage、backend consistency | 正式 schema / 真实入口行为；编译成功不代替执行目标用例 |
| 架构与状态协议 | state-protocols、四 foundation contracts、AI conformance | 各自 candidate receipt；一般 tooling pass 不替代 |
| 覆盖率 | frontend、Rust slices、API shards + merge | 必需 merge 成功并生成候选证据；任意 fail/cancel/必需 skip 不得绿灯 |
| 依赖 / 容器安全 | security、container images | 检测仍运行；第三方基础版本漏洞与 warning 本次仅记录 |
| UI、响应式、真实页面流程、活跃安装 | 本次未进行截图、浏览器或实际部署验收 | 未验证；自动门禁不能推出用户实际启用宿主/插件组合已健康 |
| 死代码 / 抽象维护性 | hygiene、churn、消费者 / 规则核对 | 已证实遗留 `.orig` 删除；没有全项目调用图/覆盖证据，不批量删公共入口或据命名判死代码 |

最终 PASS 需要同一冻结候选的 workflow success、aggregate `status=passed/exitCode=0`、准确 commit 与所有必需 job / receipt。最终候选和结果由 Root 的集中 QA 证据结算，本报告中的开发修复不自称已通过。

## Advisory Debt And Prevention

基线 `repo-hygiene` 有 294 warnings / 0 errors：96 file-size、68 test-file-size、69 debt markers、33 directory pressure、16 weak assertions、9 alias markers、3 duplicate suite titles。i18n 有 272 unused-key warnings；其中已确认资源 split 误报，剩余 key 需以修复后的报告区分真实无引用、动态引用和外部渲染入口。不能据旧总数批量删 key。

8 个实际兼容标记尚有废弃期限（2026-09-30、2026-10-31、2027），需要 owner 按期核查；scanner 自身的第 9 个误报已修正。不同文件共享 describe 名称不自动代表重复测试，3 个信号保持 advisory。上述 warning 不是“全项目健康”的证明，也不自动要求为本任务大规模拆文件。

AI 后续开发应做三件事：修改持久化布局时枚举生产查询和 fixture 消费者，并在历史 schema 验证原迁移；新增源码模块/格式/lazy owner 时同时检验 gate discovery 的合法正反例；交付前冻结 source 与 paired-source 身份，按实际 transport 与可观察 commit/terminal 边界取证。收益是减少漏检、误报与测试换产品；停止边界是产品语义、权限、归档格式或用户数据 contract 改变。

热点报告未在本地调用自动 scanner，遵守本次全部门禁线上执行约束；本报告频次由只读 Git 历史取得。后续维护性改善应使用实际消费者、事务/权限职责和可验证风险排序，不以文件长或单调用方作为删除依据。
