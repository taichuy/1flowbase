---
updated_at: 2026-09-22 12
status: active
decision_policy: verify_before_decision
---

用户已批准：LLM 下 Tool、压缩轮次、Subagent 活动归入轨迹，移除工作流主视图重复活动分组；工作流主节点及输入/处理/输出保留。活动内真实子节点、分支、来源关系必须保持可访问，不能因去掉容器丢弃后代。

原因：用户希望精简重复UI，同时此前明确反对破坏节点关系和I/O。既有trace projection才拥有route/branch/subagent关系，Native/client协议事件不能据时间邻近重建这些事实。优先复用已有日志/精确IDs，不增加第二套存储、不改供应商插件职责。

执行者：Root负责集成；当前阶段为dev前端收束。期限：无固定日期，用户新指令覆盖后失效。后续任务先核对当前代码与最新用户要求。
