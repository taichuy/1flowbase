---
title: "接口生命周期先验收，三级插件开放后审计"
memory_type: project
created_at: "2026-09-07 12"
updated_at: "2026-09-07 22"
decision_policy: verify_before_decision
status: active
tags: [interface-lifecycle, planning, github-issues]
---

用户授权Root继续接口生命周期实现，P6/P7先完成，集中验收后推送GitHub Actions，验证迁移前后输入/输出/状态/error/权限等价。活动工作区固定 /home/taichuy/git/1flowbase_latest，beta；用户明确要求提前合回beta并直接工作，覆盖旧隔离/pre-QA集成规则。无需重复申请实现/有限修复/push授权。无固定截止日期；以实际验收证据完成为目标。

唯一当前执行真值：https://github.com/taichuy/1flowbase/issues/1998 ，Delivery #1999/#2000/#2001。规范：https://github.com/taichuy/1flowbase/wiki/Request-Architecture-and-Invocation-Lifecycle-CN 。当前本地manifest与QA handoff位于tmp/test-governance/1998/test-batch.json及最新qa5-beta-handoff.md，恢复时先核对这些和git/线上状态，不重启旧Packet。

阶段顺序：接口生命周期管理 → HostExtension/RuntimeExtension/CapabilityPlugin三级插件可时空组合性开放。P6a已撤下且从未实施；空扩展plan合法，适用绑定计划不能绕过；受控Kernel证据不能冒充生产插件加载。完整Outbox/PluginData及插件装配另行审计，延期不等于通过。

P1–P7及原有限修复已在beta集成。用户随后授权修正后端Seed并补齐在线协议，前端不处理；Seed恢复固定官方发布字节，当前候选66911e14aa5cfdb983b3680b96fb41312f50caad。2026-09-07独立在线协议run34131244984成功，唯一fresh QA结算AC009按已批准有限第一阶段场景PASS_WITH_LIMITS。AC001–007明确复用8efaae3的源/fixture一致证据，不能冒充本候选重跑；当前Seed本地14+2/CI解码与在线协议实际通过。完整证据及限制：tmp/test-governance/1998/ac009-current/final-qa.md、execution-receipt.json、baseline-contract-map.md。当前verify34129516604仍因既有schema/前端失败；历史PG/coverage没有本轮重跑。Root与三个Delivery保持open/phase:qa，等待用户只验收Root，无活动开发/测试/自启服务。后续不得把声明16oracle/20error/12count_tokens当作实际线上矩阵；characterize282中225经Gateway、57直连mock WS，真实Gateway WS仅独立成功/错误2例。当前结论不是全接口/全输入空间或性能等价证明。

既有schema4、前端、official Seed integrity、PG旧升级fixture债务已单列；不要擅加scope/migrations、更改翻译或跳过完整性检查。最终结论须区分当前接口回归与全局门禁旧债。旧隔离tree /home/taichuy/git/git_worktree/interface-lifecycle-1998只读保留，结束前按用户规则决定清理；私有memory不提交。


后续批准（2026-09-07）：用户确认继续补齐已列覆盖缺口，含create字段顺序/返回落盘ID/完整授权与错误、Compact原false能力、真实Gateway WS并发/断开/中断/协议错误、完整20行上游错误矩阵，以及schema分类、旧PG升级fixture、CI空Seed过滤器。Root仍#1998，当前Control Ledger已更新；新增执行以Root最新正文为准。主beta保持不变，隔离assembly在/home/taichuy/git/git_worktree/interface-parity-followup（codex/interface-parity-followup），集中一个fresh QA，本地不跑重型Cargo，候选临时分支CI取证后再合回beta。新范围验收尚未完成，不能沿用上轮有限PASS提前结算。无固定截止日期。


2026-09-08续：隔离候选9a2957c6a集中CI34138617197仅WS lifecycle3行失败；Rust新增23/9/4/2与真实error20行24尝试通过，schema本地0 findings。Root最新F1/F2修正保持原业务语义：同provider池本来串行；连接断开!=显式业务cancel，新增确定性terminal前barrier和Nativecancel分开取证；修复已建立typed stream的completion因bridge abort被丢弃（独立owner收尾），不新增自动业务取消。失败明细必须保留。尚未合beta；以#1998当前fix batch为准，严禁把目前候选说成完整通过。


## 2026-09-08 01 本轮结算

Root已完成批准的后端一致性补验、认证schema分类及旧PG/Seed测试前置修正，b1135c206已合回并推送beta。同SHA CI34143783378与fresh QA2通过；报告位于主工作区tmp/test-governance/1998/parity-followup/qa2/final-qa.md。发现的既有WebSocket收尾owner缺口已修复；断连与显式业务取消保持分离。Root/Delivery仍open等待统一用户验收；前端、全仓verify、性能等价、完整Outbox/PluginData仍不在本轮。后续判断先查当前Issue/代码/报告，不把有限范围通过泛化为所有接口完全一致。

## 2026-09-08 08 阶段转入插件架构研究

用户表示对应接口生命周期调整已完成，要求使用 problem-framing 开始研究三级插件时空可组合性架构，重点为接口生命周期各阶段接入、事件注册与通知；随后明确研究范围包含插件自行定义事件并供其他插件订阅。动机是基于已有接口生命周期继续开放真实插件组合能力。本轮仅研究与方向对齐，不代表已批准实现、开放普通插件全局扩展点或关闭既有 GitHub Issue；无固定截止日期。后续不能沿用第一阶段“暂不研究插件开放”的范围，也不能把包含插件间事件理解为已批准具体命名空间、交付或升级方案。

用户进一步要求先重新判断三级分类本身是否必要，指出最初动机包含规避 Rust native so/dll 反复加载卸载时 TLS/static 与依赖全局状态风险；当前主要开发 RuntimeExtension，认可可信 HostExtension 随宿主整体重启的边界，质疑 CapabilityPlugin 的独立价值。此时需区分消费/贡献语义与执行/回收机制，不能仅复述现有分类便宣称三级合理。

随后用户明确认可“HostExtension + 受管插件”两种治理边界：不再把 RuntimeExtension 与 CapabilityPlugin 作为顶层互斥分类，供应商、节点、组件、Hook、事件等作为贡献类型，执行方式与激活作用域分别建模；可信 native HostExtension 继续随宿主重启，现有 Rust 子进程可复用，不因分类调整引入 WASM/Lua。最终类型名称和分类迁移尚未批准实施。用户继续提出是否需要统一技术总线管理插件/事件的注册、时机和通知获取；当前在对齐 Extension Bus 的治理职责与各执行/交付通道边界。

用户认可统一总线建议并要求全面研究可借鉴的成熟架构、算法、数据结构与数学建模。研究入口为 docs/architecture/plugin-composability-research.md（研究建议，非实现或形式化验证通过）：覆盖 OSGi/VS Code 声明与生命周期、typed graph/约束、策略交集与读写集、不可变快照、乘积状态机/TLA+、事件偏序、Outbox/Inbox、fencing、Little 定律与背压，明确各模型假设和适用边界。具体模型选择、协议、迁移与执行计划仍待确认；不把“认可建议”扩展为实现授权。
