# 业务不变量优先的代码与门禁精简（2026-11-03）

发布比较基准为 `v0.6.1`（`842605b5e`）；原候选共同祖先为 `5264a3cbd`，启动保护基线为 `97288b2c7`，验收期间新增保护基线 `c31ff2e0f`（调试消息和日志入口）已合入并保留。本次集成必须保留最新保护基线的所有业务提交。本次四个独立修改分别属于业务读模型、积分命令、Rust 静态扫描与仓库治理扫描，不能以扫描器通过代替业务行为验证。

## 业务模型与保留条件

**认证方式投影**（`control-plane/src/auth/methods.rs`）：对于同一不可变 snapshot 中按原顺序排列的外部绑定 `i`，令 `p_i = entry_enabled_i ∧ authority_i ∈ available_authorities`、`b_i = [state_i == bound]`、`U = Σ b_i · p_i`。则 `can_unlink_i = b_i ∧ (password_usable ∨ U > [p_i])`。非 bound 的 blocked reason 优先为 `external_identity_unlinked`，其次才是 `last_usable_authentication_method`。旧算法对同一绑定重复查询成员资格；新算法只计算并写入 DTO 一次，再用 DTO 求和与回填。只要 snapshot 不变，逐项 `p_i` 恒同，`U` 与最终 DTO/顺序因而恒同。解绑事务仍独立进行数据库原子重检，列表展示从不替代该重检。

**积分已验证命令**（`control-plane/src/billing.rs`）：`execute_verified_plugin_request` 的普通六命令路径有明确前置条件 `V(r,P)`：命令已识别、许可已检查、请求字段已校验、生成的 actor_user_id 为 None。到内层旧检查之间没有 await 或可变权限写入。故在 `V` 下，旧内层检查不会执行数据库／审计副作用；直接调用相同 repository、传入相同字段和 `actor_plugin_id`，得到相同返回值、错误与副作用轨迹。外层拒绝审计、幂等冲突、审计失败覆盖规则以及公开 `execute_plugin_command` 的独立校验均保留。该内部前置条件不允许推广到公共入口。

验证器分别采用真实 DTO、解绑集成不变量、六命令及错误/审计轨迹；用独立真值表和拒绝 fixture 作为受控反例。未执行的 fixture 不能标为通过。

## 门禁的输入集合与复杂度

**Rust 静态扫描**（`scripts/node/check-rust-backend/core.js`）：`isSkippedRustPath` 是已经存在的唯一排除谓词。提前执行它只减少原本必然返回空 findings 的文件读取，且 `scanRustSource` 保留自身检查；扫描目标、finding 顺序、严重级别、review SHA 校验和 fail-closed 均不改变。目录枚举复杂度不变；被跳过的正文读取字节数由 `B_skip` 降至 0。基线磁盘统计：3,105 个 Rust 文件，总 38,624,231 字节；原本跳过的 1,464 个文件，共 17,733,003 字节。这是文件字节与操作次数，并非实测 I/O 延迟。

**repo-hygiene**（`scripts/node/repo-hygiene/core.js`）：文件清单先冻结；每个源文件只读取一次，逐文件立刻扫描并将相同正文传给既有的可迭代重复标题归集器。原有 findings 仍按 tracked artifact → 逐文件 → 目录压力 → 重复标题顺序输出；异常和非法输入继续失败。设文件清单开销 `F`、标题索引 `T`、所有正文总字节 `B`、最大单文件正文 `L_max`，扫描正文同时驻留从 `O(B)` 降为 `O(L_max)`，总工作集由 `O(F+T+B)` 变为 `O(F+T+L_max)`。原始清单为 5,848 文件、65,356,347 字节，最大文件 5,386,674 字节。峰值 RSS、GC 与墙钟变化必须另在相同进程预算和输入 SHA 下实测，不以理论边界冒充实际收益。

## 验收与资源

遵循现有 Level 1 快速静态 / 定向测试、Level 2 PR 契约 / 集成 / 构建复用、Level 3 完整回归 / 发布收据；仅增加调用现有完整门禁的 PR opt-in 派发入口，不新增重复业务验收 owner，不改变原有覆盖率阈值或原来应阻断的错误。比对完整有序 DTO / findings、repository/审计写轨迹以及至少一个会被旧门禁捕获的坏 fixture，缺陷检测保留率必须为 100%（分母为实际执行的非空反例集合）。

同一时刻只允许一条重型 Cargo / Rust 测试；本地资源遵循仓库 .1flowbase.verify.local.json 与宿主 CPU 60%、内存 8GiB 的用户资源上限，且不得超过仓库 `api/AGENTS.md` 与共享预算入口计算的有效容量。提交时分别报告基线、候选 SHA、测试用例数、P50/P95、Runner 分钟、RSS/OOM 和净行数；环境或数据不一致时不宣称性能改善。外部完整 CI 的 candidate-bound 收据不能由本地定向测试替代。

## 历史基线恢复的 review 证据

2026-11-03 的合并前基线已有 Rust review evidence SHA 不匹配。逐文件核对发现 `projections.rs` 对比 `v0.6.1` 仅添加同步写入 DTO 的 `ui_registration` 字段，不改变 `load_plugin_settings_templates` 的阻塞路径、`spawn_blocking` 控制流或审查理由。因此更新原 review 证据 SHA，以继续保留 SHA 绑定与后续变更时的 fail closed。该修正不新增 review 条目，不更改原有规则或 suppress 范围。

`sync-js-runtime` 是当前 workspace 中已有的无内部依赖 crate，改由 runtime-storage 的 check/Clippy 与 runtime-storage-fast 的直接包测试覆盖。既有 workspace inventory 负例仍必须能发现任何新出现的未归属 crate。Rust `cargo metadata` 源码检查改为验证分开的 `command.arg("metadata")` 和 `--locked`/`--offline` 选项，保留移除任一选项时的失败条件。

## 2026-11-03：既有功能清单与 CI 运行前提回归

- Console 权限投影的源真值是已编译 `ConsoleOperationRegistry`：继原 445 条固定清单之后，个人资料/偏好写入 3 条、通知状态写入 3 条、临时文件链接 1 条独立可授权操作使其成为 452 条。保持真实 `(method, normalized_route)` 的确定性排序与全量严格相等断言；不得通过回滚生产权限策略或者放宽断言收绿。
- Core 文案注册表已包含“Personal account”及其描述，消费者固定清单从 86 条调整为 88 条；官方 seed 是固定外部版本，新增 7 项接口的 summary/description 尚未在 768 条消息键中发布，因此仅按具体 operation ID 使用已存在的 pending-official-release 路径，不修改 seed 字节。
- PostgreSQL 插件组件测试使用真实 Host TSX 编译器。CI Rust-only runner 必须为存储集成分片提供既有 pnpm、Node 及锁文件规定的 TypeScript、page-runtime，不绕过 duplicate-import 负例或引入新包；源测试所需的审计 actor 必须在隔离 schema 中真实存在，外键约束不变。
- 本地低成本复核：业务接口与字符串源码锚定、Rustfmt、Git diff 检查、真实 Host TSX 编译+重复 import 拒绝；Node gate 测试 84/84，`/usr/bin/time` 单进程 maxRSS 246176 KB。以上不代替 Rust 真实数据库执行或候选 SHA 的远程完整门禁。

## 可选的完整 PR 门禁触发

由于 GitHub Verify 是 Level 2，完整 Quality Gate 仍为受控的 Level 3 手动工作流。为避免每次 PR 更新都重复占用完整回归 runner，仓库内同源 PR 仅在维护者添加 `quality-gate` 标签后调用原有 `workflow_dispatch`；标签保留时，候选后续提交触发下一轮完整验收。派发作业不检出或执行 PR 业务代码，仅使用 GitHub runner 现有 CLI、最小 `actions:write` 权限；叉仓 PR 不可触发。

派发前比较事件绑定 SHA 与远端分支最新 SHA；完整门禁的 `resolve-quality-gate-target` 再检查 `candidate_sha` 与实际检出 HEAD 相同，任一变化立即失败，防止用旧的通过收据验收新候选。React Doctor 的 `react_doctor_base` 取目标受保护分支在 PR 事件中的基线提交，不以缺失或过期的定时扫描收据推断本次 PR 差异。现有完整门禁组件、阈值、负例、环境及覆盖范围保持不变。

## 固定点与请求栈的相邻回归

只读变量缓存的投影由 `AgentFlowCanvasFrame` 拥有：权限关闭时仍把每项标为 `isReadOnly`，但输入分组与权限不变时保持相同引用，避免 `DebugVariablesPane` 的选中项 effect 每次向父组件写入新对象而持续渲染。使用已有 `useMemo`，依赖真实权限布尔值和服务端运行态分组；权限变化、运行态数据变化仍重新投影，不修改原分组。真实 Assembly 回归覆盖选择第二项、无关消息更新、权限授予/撤销、固有只读项及新分组刷新。

调用内核的 unary `invoke` / `invoke_with_dispatch_target` 和业务 `start_published_flow_run` 直接以同步工厂返回原有 pinned boxed inner future，去掉只负责再次等待的外层 async 状态机。输入仍由同一冻结 envelope / command 传入，内层业务代码仍在首次 poll 才执行；结果、错误、poll/wake、取消、审计和唯一 Drop owner 保持。已 poll 的调用仍只有原来一次装箱；从未 poll 的 future 会提前分配并随 drop 释放。这减少调用方捕获参数的外层状态与一层 poll 包装，不增加 task 或改变 scheduler；不将对象大小当作整个 poll 栈的字节数，也不扩大既有 2 MiB 真实路由测试栈。独立标准库 Rust 验证器在 debug 与优化构建下检查未 poll 释放、取消、成功/错误及 target owner 的 12 个有限调度组合；生产栈与协议行为仍由真实路由回归验证。

独立有限模型检查认证 299,592 个组合及积分 896 个输入/返回/审计轨迹组合，均无反例；认证的任意有限绑定数等价性由逐项替换和前缀和归纳建立。此证据不等于运行 Rust 实现，也不替代真实 PostgreSQL 原子解绑、六命令及拒绝/审计行为测试。认证成员查询减少量为 enabled 且 bound 的绑定数；积分有效六命令各消除一次重复权限映射/集合查找与 actor 检查。两项渐近复杂度、数据库查询及持久化空间不变，不将常数项简化表述为算法阶数下降。


## 2026-11-04：共享执行分派与语义续接

真实 coverage 二进制、原 2 MiB 路由线程的首次 SIGSEGV 取证确认：Chat / Anthropic 的累计 poll 栈耗尽；SQLx 位于最后触碰保护页的位置，不能据此归因为数据库库缺陷。独立核对大帧序言与相邻 SP 区间，其中空可见内部工具路径仍经过的工具循环帧占 196,128 字节。`visible_internal_llm_tools` 现在同步选择唯一执行 future：空工具直接装箱原 Provider round，非空工具装箱原循环；不修改 Provider 调用、循环、回调及状态写入。纯配置解析提前至构造时，增加一次所选 future 的堆分配，未 poll 的 future 也会解析配置并分配；不增加 task，不改变业务首次 poll 才执行的边界。独立纯标准库模型在 debug / 优化构建各检查 72 个成功、错误、Pending、取消及唯一 pool owner 调度；实际栈收益由相同真实路由验证，不能以 future 大小替代。

活跃 semantic callback 优先于末尾压缩控制标记。control-plane 对当前已证明续接后缀的末尾单项 marker 复用 V2 精确形状验证，其余 context 继续使用原 semantic 校验（保留省略 `type` 的合法消息）；完整 wire input 与 history proof 保留 marker，Provider messages 排除它。runtime 消费者对同一个合法末 marker 取 context，保留原输入／消息长度、顺序与完整工具结果集合检查，并拒绝 context 或消息中的残留 marker；错误仍在局部组装阶段拒绝，不写回 variable pool。回归保留原真实 SSE 终态断言，补充无 marker / 有 marker 的完整 pool 等价及错误输入原子拒绝。协议 history 与 Provider conversation history 有各自 owner，不能为使长度相等丢弃持久化的 wire 控制项。
