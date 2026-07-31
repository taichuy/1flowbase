---
created_at: 2026-07-31 17
updated_at: 2026-07-31 20
memory_type: feedback
decision_policy: direct_reference
feedback_category: repository
scope:
  - web/app/src/features/agent-flow
  - api/crates/orchestration-runtime
  - api/crates/control-plane/src/orchestration_runtime.rs
keywords:
  - agent-flow
  - single-node debug
  - node preview
  - variable cache
  - branch isolation
---

# AgentFlow 单节点调试必须与工作流拓扑隔离

规则：`运行当前节点 / 调试当前节点` 的目标由用户选择，只执行该目标节点；不得回放上游 `If / Else`、变量赋值或其他节点，也不得以目标在当前输入下是否路由可达作为执行门禁。只有全局 / whole-flow debug 才消费工作流拓扑与分支激活状态。

规则补充：单节点引用变量时，仅从调试变量缓存读取目标节点直接引用的变量；缓存缺失则弹出表单让用户手动输入。不得因为上游控制节点或状态写节点存在，就额外收集其依赖、推导状态或形成隐式上下游依赖。

规则补充：单节点已经开始真实执行后，无论成功还是数据源 / Provider / 节点运行错误，都必须持久化本次 `flow_run + node_run`，向前端返回可展示的 Last Run，并自动切换到“上次运行”。Toast 只用于尚未形成运行记录的请求、权限、输入确认或编译前置失败；不得用 Toast 替代已经发生的节点执行错误详情。

原因：单节点调试的激活 authority 是用户显式选择，目标是隔离验证节点自身配置、输入、输出与真实运行错误。把 whole-flow 路由或上游状态回放混入该入口，会让未命中分支的节点无法调试，并把节点真实错误替换成拓扑可达性错误。

适用场景：节点预览输入计划、节点调试 API、preview executor、变量缓存恢复、缺失变量表单、Last Run 持久化、执行错误落点，以及新增分支或变量赋值运行语义时的回归测试。
