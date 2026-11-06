---
memory_type: project
topic: 事件中心与事件工作流本地实现
summary: 已完成本地dev集成和开发服务重启；325个定向用例及真实双端页面/重启/样式证据通过。后续以当前源码、架构文档和验收报告核实。
created_at: "2026-11-06 02"
updated_at: "2026-11-06 05"
decision_policy: verify_before_decision
status: complete
---

- Root 已在 taichuy-dev 的 `/home/taichuy/git/1flowbase` 完成本地实现并合回 dev，7800 API/3100前端已恢复、认证事件接口和页面检查通过；7600观察服务未动。用户未指定截止日期。
- 目标与动机：事件目录、瞬时队列观察、三时机 Workflow 与通信自定义指令连通；复用已有总线、编排和共享 DataTable，保持交接与消费者业务结果的职责边界。
- 已确认边界：消费=dequeue/责任交接；事件、队列、动态仅ephemeral，配置持久。通信指令发布事件，通知由已有工作流动作发送。完整规则见反馈记忆和 `docs/architecture/event-center/README.md`，不扩大为可靠Outbox/重放平台。
- 验收与执行记录：`tmp/test-governance/event-center-20261106/qa-cycle-6/qa-report.md`、`root-plan.md`；325个有效定向测试、实际进程重启、双端页面和正式样式门禁通过，主分支本地运行证据为同目录 local-runtime-api 和 local-final-capture.json。
- 为遵守本地8GB约束，大型contracts测试在本地OOM后转到用户已授权的GitHub Actions；成功补证run42107540936。临时codex/event-center已push用于CI；dev没有push，没有生产部署或PR。后续状态需重新查git/运行态，不凭旧SHA推断。
- 主环境未提交的.env加入显式开发事件预算，生产代码没有容量默认值；首次进入可创建custom事件，再在Workflow选择事件触发。系统/插件声明只读，只有已有来源适配能力才开放时机。
