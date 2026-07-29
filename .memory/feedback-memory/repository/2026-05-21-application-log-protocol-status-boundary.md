---
memory_type: feedback
feedback_category: repository
topic: application-log-protocol-status-boundary
summary: 应用日志继续使用 compatibility_mode 表达映射协议；统一 Native 入口不得另造 ingress_protocol，而应由可信协议 adapter 恢复赋值。
keywords:
  - application logs
  - conversation log
  - protocol
  - status
  - i18n
created_at: 2026-05-21 11
updated_at: 2026-07-29 08
last_verified_at: 2026-07-29 08
decision_policy: direct_reference
scope:
  - web/app/src/features/applications
  - web/app/src/features/agent-flow/components/debug-console
---

# Application Log Protocol Status Boundary

## 规则

应用运行 `compatibility_mode` 字段需要展示时，UI 文案可叫“协议”，但前后端接口和代码字段名应保持 `compatibility_mode`。统一 AI Native 入口后也不得为同一事实另造 `ingress_protocol`：公开请求不能自行提交 `compatibility_mode`，但可信映射协议 adapter 必须把其 `TranslationProtocol` 映射回既有 run 字段。展示位置优先放在 `/applications/:id/logs` 表格列和对话日志 `详情 -> 元数据` 中。不要顺手调整运行状态文案或 `completed` 这类状态映射，状态多语言后续由前端国际化统一处理。

## 原因

用户明确确认 `compatibility_mode` 原本就是映射协议，不应因统一 Native 入口而新增同义字段。客户端不可伪造该字段与服务端不可记录该字段不是同一命题；正确 owner 是已经知道入口协议的 adapter。复用既有字段还能维持日志 DTO、监控分组和 Claude Code 运行关联的一致语义。协议展示和状态本地化属于两个不同交付点，混在一起会扩大变更范围。

## 适用场景

修改应用日志表格、运行详情浮窗、对话日志元数据、公开 API / 兼容协议运行展示时命中。
