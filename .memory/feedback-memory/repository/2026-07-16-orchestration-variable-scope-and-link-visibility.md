---
memory_type: feedback
feedback_category: repository
topic: 编排变量作用域与连接可见性
summary: sys/env/conversation/trigger 等运行级命名空间按各自读写策略对整次运行全局有效；节点输出只对沿连接器可达的下游节点可见，前后端与首次运行/恢复路径必须执行同一契约。
keywords:
  - orchestration runtime
  - variable scope
  - connector reachability
  - conversation variables
  - system variables
created_at: 2026-07-16 12
updated_at: 2026-07-16 15
decision_policy: direct_reference
scope:
  - api/crates/orchestration-runtime
  - api/crates/control-plane/src/orchestration_runtime
  - web/app/src/features/agent-flow
---

# 编排变量作用域与连接可见性

规则：`sys`、`env`、`conversation`、`trigger` 等运行级命名空间在整次运行内全局可见，并分别遵守系统只读、环境快照只读、会话变量可读写、触发上下文只读的既定策略；不得依赖节点是否与 Start 直接相邻。

规则：普通节点输出不是全局变量。只有来源节点沿图连接器可达当前节点时，当前节点才能通过 selector / template / condition / context selector 读取该输出；该约束必须由后端编译与运行时守住，不能只靠前端变量选择器校验。

规则：首次执行、暂停恢复、节点预览、可见内置 LLM 工具分支和发布 API 必须使用同一作用域与可见性契约。若分支采用隔离状态，隔离边界必须显式且同步/暂停恢复行为一致；不能因是否发生 callback 改变变量副作用。

规则：等待态与终态 Answer 必须属于真实激活分支。候选 Answer 只能由 active set 或当前等待节点实际引用关系选出；未激活分支即使只有静态文本，也不得创建 node run、覆盖输出或进入协议投影。

原因：编排的正确性来自“运行级上下文全局稳定 + 节点数据沿连接关系传播”这两个不变量。把节点输出放进无边界全局池会产生跨断开子图读取；只检查直接依赖又会截断合法的传递上游；多套执行循环会让同一图在首次运行与恢复后产生不同结果。

适用场景：调整变量池、selector、templated text、If/Else、会话变量赋值、LLM system/history、节点预览、checkpoint/resume、内部工具分支、工作流/Agent Flow 编译与执行时命中。
