---
created_at: 2026-08-24 00
updated_at: 2026-08-24 00
memory_type: feedback
feedback_category: communication_scope
decision_policy: direct_reference
scope: 1flowbase architecture diagrams and explanations
---

# 架构图必须展示 crate 层级

- 规则：讨论 1flowbase 后端架构时，图与说明必须同时覆盖 Cargo crate 层级、直接依赖方向、部署单元和插件执行边界；不能只画插件分类。
- 规则：整体架构图要把请求主链与扩展主线同时画出。请求主链是 Gateway（前缀、认证授权、分发）→ Application / Control Plane → Domain / Runtime → Infrastructure；HostExtension、RuntimeExtension、CapabilityPlugin 通过统一扩展图纵向贯穿各层，内置扩展也走同一注册机制。
- 规则：基础设施存储只按 Durable、Ephemeral、Object 三种语义分类；PostgreSQL 是 Durable 的实现，不是并列的第四层。现有 `publish-gateway` 不得误画成入站 Gateway，必须按其真实职责归入供应商/模型调用路由。
- 规则：架构图还必须区分启动注册流、同步请求流和业务数据流。Gateway 是统一请求入口与通用拦截链；认证/入口授权是可阻断的同步 Pipeline/AOP 阶段；Controller 声明 method、path、operation 与 policy，并编译进统一路由/接口注册表。不要用一句“Gateway 调 Application”省略这些数据变换，也不要把认证误画成异步 EventBus。
- 规则：重构架构必须从已确认的请求线路自上而下映射现有 crate，再决定保留、合并或移动职责；不要从预设的新 Contract crate、Issue Tree 或编译优化技巧反向拼架构。讨论阶段不得擅自更新 Issue 或进入实施。
- 原因：用户需要用 crate 编译单元与职责关系判断模块化单体架构，只描述 HostExtension / RuntimeExtension / CapabilityPlugin 不足以支撑决策。
- 原因：只画横向分层会把扩展退化为旁路，只画插件会丢失系统主线；两条轴必须同时成立，才能表达用户要求的时空可组合性和清晰依赖方向。
- 原因：用户需要从图上直接看到 RequestEnvelope 如何产生 ActorContext、如何按注册路由分发、如何进入 Command/Query，以及结果如何选择 Durable/Ephemeral/Object 端口；否则层级名称正确也无法解释真实运行机制。
- 原因：先造抽象再迁移会把现有耦合换成新名字，偏离“不影响功能和用户完整性”的重构目标；请求主线才是职责 owner 与依赖方向的第一真值。
- 适用场景：后端容器合并、crate 拆分、依赖图、plugin-runner 边界、编译性能和架构现状说明。
