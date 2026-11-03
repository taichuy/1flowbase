# 业务不变量优先的代码与门禁精简（2026-11-03）

实施基线：发布版本 `v0.6.1`（`842605b5e`），开发候选父提交 `5264a3cbd`。本次四个独立修改分别属于业务读模型、积分命令、Rust 静态扫描与仓库治理扫描，不能以扫描器通过代替业务行为验证。

## 业务模型与保留条件

**认证方式投影**（`control-plane/src/auth/methods.rs`）：对于同一不可变 snapshot 中按原顺序排列的外部绑定 `i`，令 `p_i = entry_enabled_i ∧ authority_i ∈ available_authorities`、`b_i = [state_i == bound]`、`U = Σ b_i · p_i`。则 `can_unlink_i = b_i ∧ (password_usable ∨ U > [p_i])`。非 bound 的 blocked reason 优先为 `external_identity_unlinked`，其次才是 `last_usable_authentication_method`。旧算法对同一绑定重复查询成员资格；新算法只计算并写入 DTO 一次，再用 DTO 求和与回填。只要 snapshot 不变，逐项 `p_i` 恒同，`U` 与最终 DTO/顺序因而恒同。解绑事务仍独立进行数据库原子重检，列表展示从不替代该重检。

**积分已验证命令**（`control-plane/src/billing.rs`）：`execute_verified_plugin_request` 的普通六命令路径有明确前置条件 `V(r,P)`：命令已识别、许可已检查、请求字段已校验、生成的 actor_user_id 为 None。到内层旧检查之间没有 await 或可变权限写入。故在 `V` 下，旧内层检查不会执行数据库／审计副作用；直接调用相同 repository、传入相同字段和 `actor_plugin_id`，得到相同返回值、错误与副作用轨迹。外层拒绝审计、幂等冲突、审计失败覆盖规则以及公开 `execute_plugin_command` 的独立校验均保留。该内部前置条件不允许推广到公共入口。

验证器分别采用真实 DTO、解绑集成不变量、六命令及错误/审计轨迹；用独立真值表和拒绝 fixture 作为受控反例。未执行的 fixture 不能标为通过。

## 门禁的输入集合与复杂度

**Rust 静态扫描**（`scripts/node/check-rust-backend/core.js`）：`isSkippedRustPath` 是已经存在的唯一排除谓词。提前执行它只减少原本必然返回空 findings 的文件读取，且 `scanRustSource` 保留自身检查；扫描目标、finding 顺序、严重级别、review SHA 校验和 fail-closed 均不改变。目录枚举复杂度不变；被跳过的正文读取字节数由 `B_skip` 降至 0。基线磁盘统计：3,105 个 Rust 文件，总 38,624,231 字节；原本跳过的 1,464 个文件，共 17,733,003 字节。这是文件字节与操作次数，并非实测 I/O 延迟。

**repo-hygiene**（`scripts/node/repo-hygiene/core.js`）：文件清单先冻结；每个源文件只读取一次，逐文件立刻扫描并将相同正文传给既有的可迭代重复标题归集器。原有 findings 仍按 tracked artifact → 逐文件 → 目录压力 → 重复标题顺序输出；异常和非法输入继续失败。设文件清单开销 `F`、标题索引 `T`、所有正文总字节 `B`、最大单文件正文 `L_max`，扫描正文同时驻留从 `O(B)` 降为 `O(L_max)`，总工作集由 `O(F+T+B)` 变为 `O(F+T+L_max)`。原始清单为 5,848 文件、65,356,347 字节，最大文件 5,386,674 字节。峰值 RSS、GC 与墙钟变化必须另在相同进程预算和输入 SHA 下实测，不以理论边界冒充实际收益。

## 验收与资源

遵循现有 Level 1 快速静态 / 定向测试、Level 2 PR 契约 / 集成 / 构建复用、Level 3 完整回归 / 发布收据；不增加 CI 工作流、不改变原有覆盖率阈值或原来应阻断的错误。比对完整有序 DTO / findings、repository/审计写轨迹以及至少一个会被旧门禁捕获的坏 fixture，缺陷检测保留率必须为 100%（分母为实际执行的非空反例集合）。

同一时刻只允许一条重型 Cargo / Rust 测试；本地资源遵循仓库 .1flowbase.verify.local.json 与宿主 CPU 60%、内存 8GiB 上限。提交时分别报告基线、候选 SHA、测试用例数、P50/P95、Runner 分钟、RSS/OOM 和净行数；环境或数据不一致时不宣称性能改善。外部完整 CI 的 candidate-bound 收据不能由本地定向测试替代。

## 历史基线恢复的 review 证据

2026-11-03 的合并前基线已有 Rust review evidence SHA 不匹配。逐文件核对发现 `projections.rs` 对比 `v0.6.1` 仅添加同步写入 DTO 的 `ui_registration` 字段，不改变 `load_plugin_settings_templates` 的阻塞路径、`spawn_blocking` 控制流或审查理由。因此更新原 review 证据 SHA，以继续保留 SHA 绑定与后续变更时的 fail closed。该修正不新增 review 条目，不更改原有规则或 suppress 范围。

`sync-js-runtime` 是当前 workspace 中已有的无内部依赖 crate，改由 runtime-storage 的 check/Clippy 与 runtime-storage-fast 的直接包测试覆盖。既有 workspace inventory 负例仍必须能发现任何新出现的未归属 crate。Rust `cargo metadata` 源码检查改为验证分开的 `command.arg("metadata")` 和 `--locked`/`--offline` 选项，保留移除任一选项时的失败条件。
