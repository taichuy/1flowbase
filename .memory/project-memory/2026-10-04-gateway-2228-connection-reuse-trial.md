---
memory_type: project
topic: 网关资源 Root2228 最终可靠优化
summary: 原overview反向关联放大已修复；连接复用及多CASE失败尝试未接受。最终0693f82c已合dev/push，161测试与33.47分钟真实长测通过，C8有限CPU稳定性收益成立。
created_at: 2026-10-04 02
updated_at: 2026-10-04 14
decision_policy: verify_before_decision
---

用户授权Root找到可复现原因并可靠优化，CPU优先且功能完整性优先；Root2228/Delivery2229、2230是任务入口。2026-10-04最终源码0693f82c已合回并push；收尾另一前端任务将本机dev/origin dev推进到762b1fe4，仅web变化，API tree6a2ec217及本任务7文件完全不变。未部署；主目录保持dev。两个自有worktree及owned测试API/plugin/PG/卷已回收，私有快照保留，7600 PID2427007/start71926973未操作。

可靠结论范围：原overview查询实际反向展开历史引用，返回1行需1305～5792buffer hits；先materialize目标metadata后375目标采样均索引点查6～10hits。按事实SQL shape减少重复校验并避免多CASE候选重规划，不改逐事实事务、锁后可见性、原文/分页/恢复/取消。单纯连接复用和之前总CPU上升候选已拒绝，不能重新当成功经验。

独立CI37176561283的12格B-C-C-B：C8/512KiB/128历史items/256deltas，含5s排空总CPU1481.33→1389.65ms/request，两顺序-3.39/-8.84%，pooled-6.19%；CV从3.3559/3.9604%到1.2006/1.2093%。C1大历史持平、小C1 p95+约1%保留；不承诺全部峰值消失。最终CI37179050635绑定latestdev assembly，161实际测试、live28项、33.468661分钟150工具真实long、58资源复算+9归档关联全部通过。

最终long两incomplete capture各5原始帧，block/checksum/序列/head及后继351/280完整帧已独立验证；原client trace明确child mailbox抢占→重连，full input保留call_id和child消息。dropped4是现有不完整标记计数，不等于已证明丢4帧；persist_failed0。2waiting_callback原分支与1local_summary_superseded取消保留真实状态，不篡改为全部完成。

资源：该long API平均单核2.36%、PSS采样峰227.27MiB；实际DB物理+40.80MiB、WAL657.30MiB分账。PSS不等于每请求allocation；无paired真实上游baseline，不从此long计算因果收益。30分钟时长本身不能决定固定聊天存储。

证据入口：`tmp/test-governance/gateway-sql-cost-20261004/final-report.md`、`overview-qa/integration/report.md`、`state.json`、`final-integration-receipt.json`；真实原始数据在`tmp/test-governance/gateway-specialization-live-20261004`。状态有效期至下次代码/依赖/环境变更；后续决策先核验当前源码与报告，不重复已完成测试或把旧binary证据套给新候选。没有待实施截止日期，本阶段完成。
