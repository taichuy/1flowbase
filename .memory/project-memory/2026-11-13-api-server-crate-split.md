---
memory_type: project
topic: api-server-crate-split
summary: Root #2824七crate已装配；用户接受reset证据并明确批准合入beta，2cd98f7d6已FF并push origin/beta；原QA与未验证功能AC仍如实保留。
created_at: 2026-11-13 12
updated_at: 2026-11-15 11
decision_policy: verify_before_decision
scope: [backend, api-server, build-memory]
---

## Approved scope
用户于 2026-11-13 确认：走平衡方案；新 crate 放 `api/crates/api/`；MCP 独立成 `api-mcp`；整理为文档计划。硬原则：不能破坏功能完整性。

## Why
`api-server` 单个 rustc 进程冷编译峰值 9.1–9.5 GiB（基线 `beta@5104e422d`），测试编译有 5 个约 9 GiB 的单元；用户希望开发与测试编译落在 8 GiB 内。主因是单 crate 25 万行（路由 19.4 万行）的单态化，不是模块数量。

## Active control
- 唯一计划真值：[taichuy/1flowbase #2824](https://github.com/taichuy/1flowbase/issues/2824) 正文/Control Ledger；D1 #2825–D5 #2829已由用户一次授权。
- 2026-11-14用户再次纠正不要重复授权；Root按既定目标和硬约束自主处理有限Owned补齐、fixture及Work Packet重切，真正改变契约/权限/硬约束时才提出新增决策。
- Scout、四面golden、7271实际测试identity基线、D1校准（库峰下降1.293GiB）与B1标准全bin基线已保留。D1–D5和fixture已装配后，唯一fresh QA1的dev2底层Cargo成功/OOM0，但实际linker4.147GiB超过3.9GiB，保守invocation也超过4；anon从B1 8.327降至3.338GiB不能抵消链接AC失败。其他集中AC未完成，未集成/推送beta。
- 六个有限fix已全部装配冻结后，唯一fresh QA2对80a933b1f执行正式全bin冷dev2：Cargo0/OOM0，链接GNU3.9277GiB、实际LLD采样3.9260GiB均>原3.9GiB。命中同根因第二次QA失败，Root已STOP_REFRAME；无QA3/继续实现，不改阈值/并发/profile/linker。自有QA target经严格metadata helper与Cargo清理完成，原FAIL/raw保留。完整测试/四面等价/运行态等余项未验证，不合入或push beta；Root活动计划与后续架构定界仍只见#2824。
- 2026-11-15用户明确接受完整reset-rust-cache预热成功结果，随后要求合入dev小更新、解决冲突并再跑同一脚本。已在隔离assembly合入固定dev f7200af95（含运行期间新增的临时JSON存储/MCP修复），最终merge 2cd98f7d6；新owner下保留全部dev变更，原CLI最终重跑全预编译成功：RSS4.07GiB/匿名3.77GiB/含cache14.06GiB，OOM0/swap0；MCP whole-body5、extension契约186、控制面契约77tests通过。临时独占端口env已恢复。该用户指令不应再被旧内部STOP当作需要重复授权；原QA失败事实与功能AC缺口仍保留，不自动声称Root完整通过。
- 2026-11-15用户明确要求合入beta；Root已将主树beta从f7200af95快进到最终候选2cd98f7d6并推送origin/beta。全程不切主树分支，两份本地私有记忆在合并时逐byte保留；复用同SHA脚本/定向测试证据，无重复完整构建。Root Ledger已记录集成；不据此伪称原未运行的完整功能AC或CI通过。
- 主工作树保持beta；唯一任务worktree `/home/taichuy/git/git_worktree/issue2824-assembly` 复用隔离assembly与各Delivery临时分支。每次恢复从Root及当前git核对输入，开发HEAD不能当最终通过候选。
- 保持7crate/AC/Delivery顺序和单一fresh集中QA。具体活动Packet、SHA、已固定的双target测试保留和标准构建量化基线补测、剩余D5闭包只见Root，不从本记忆重新调度。
- 草稿仅历史副本，活动状态只见Root。无承诺交付日期。

## Confirmed decisions（用户，2026-11-13）
保留三个 `include!` 测试根与 `api-test-selection`；第七个 crate 为 `api-console-interaction`；阈值：单 rustc 进程 ≤4.0、dev 构建 ≤7.5、测试编译 ≤8.0、链接 ≤3.9 GiB（cargoJobs=2）；单一 Root，每个 Delivery 边界变基到 beta。

## Notes
降幅数字是模型预测，未做原型验证；D1 装配后复测是校准点（下降不足 1.0 GiB 则停止 reframe）。
