---
memory_type: project
topic: 网络中心全局代理池运营能力
summary: 已确认并实现代理池成员的安全运营投影与后端连接测试；质量检测留待后续独立能力。
keywords:
  - network egress
  - proxy pool
  - connection test
  - global pool
created_at: 2026-08-22 11
updated_at: 2026-08-22 11
last_verified_at: 2026-08-22 11
decision_policy: verify_before_decision
scope:
  - api/apps/api-server/src/routes/network_center/pools.rs
  - api/crates/control-plane/src/network_egress_pool.rs
  - web/app/src/features/settings/network-center/pools
---

# 网络中心代理池运营能力已实现

网络中心保持一个系统唯一全局代理池。代理池成员现在由后端投影提供安全地址摘要、类型、地区、健康状态和持久化连接测试结果；静态 HTTP 只可显示 `host:port`，扩展节点不泄漏订阅、密码或密钥。延迟作为独立列，始终持久化最近一次测试值；未测试、旧空值与无法完成探测的记录统一为 `0ms`。

连接测试由后端以固定 IP 回显目标执行，复用现有出口租约获取/释放路径并写回成员状态、延迟、出口 IP、错误码和时间。前端只触发成员动作和展示 DTO，不接收或传递用户任意测试 URL。质量检测、批量测试和评分尚未进入此范围，后续需另行冻结指标、任务模型和插件/运行时契约。

定向证据：API library check、Network Center ACL route tests、OpenAPI operation test、代理池前端测试、API client 测试均已通过；全仓 i18n hygiene 仍有与本改动无关的既有 4 个 duplicate-value errors。
