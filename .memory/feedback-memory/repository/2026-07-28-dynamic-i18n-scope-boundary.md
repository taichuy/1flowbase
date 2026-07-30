---
memory_type: feedback
feedback_category: repository
topic: dynamic-i18n-scope-boundary
summary: 动态多语言首期只作用于 bootstrap root workspace；不建立 workspace 到 system 的翻译继承，也不因低代码 Application 消费文案而增加 application scope。未来多 workspace 应按 workspace 独立解析，不能默认跨 scope 回退。
keywords:
  - i18n
  - scope
  - system
  - workspace
  - application
match_when:
  - 设计动态多语言持久化、缓存或权限边界
  - 为低代码、Application 或公开页面设计共享配置
  - 判断一个新资源应使用 system、workspace 还是 application 身份
created_at: 2026-07-28 16
updated_at: 2026-07-28 17
last_verified_at: 2026-07-28 17
decision_policy: direct_reference
scope:
  - api/crates/domain
  - api/crates/control-plane
  - api/crates/storage-durable
  - web/app/src/features/settings
---

# Dynamic i18n Scope Boundary

## 规则

- 动态多语言首期只读写 bootstrap root workspace；不建立 `workspace -> system` 翻译继承链。
- 数据可保留明确 `workspace_id` owner，但首期 API、权限和缓存不开放多 workspace 行为。
- Application 是 workspace 内资源，不是动态多语言 scope。不得仅因低代码 Application 消费文案就增加 application scope。
- 未来开放多 workspace 时，每次请求只解析目标 workspace 自己的有效目录；是否复制官方默认或增加共享层必须重新做产品决策，不能默认跨 scope fallback。

## 原因

提前增加 system 继承、多 workspace 或 application scope，会引入当前目标不需要的权限、覆盖、发布与缓存失效语义，并改变“请求属于哪个 workspace，文案就只作用于哪个 workspace”的产品边界。

## 适用场景

- 动态多语言持久化、缓存和更新设计。
- 低代码页面或 Application 需要读取 workspace 级共享内容时。
- 从消费方身份反推数据 owner 或 scope 时。
