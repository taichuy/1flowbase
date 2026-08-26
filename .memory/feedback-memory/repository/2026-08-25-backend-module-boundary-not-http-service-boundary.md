---
memory_type: feedback
feedback_category: repository
topic: 后端模块分离不等于 HTTP 服务拆分
summary: 1flowbase 前期后端默认是单容器模块化单体；插件可执行代码可保持子进程隔离，但插件总线和插件宿主的模块边界不得自动升级为长期 HTTP 服务、第二端口或独立容器。
keywords:
  - plugin-runner
  - extension-bus
  - modular-monolith
  - single-container
  - process-isolation
  - compile-coupling
match_when:
  - 讨论插件总线、plugin-runner、后端容器、端口或部署边界
  - 拆分 Rust crate、插件宿主或执行 supervisor
  - 判断模块隔离是否需要 HTTP、微服务或独立镜像
created_at: 2026-08-25 22
updated_at: 2026-08-25 22
last_verified_at: 2026-08-25 22
decision_policy: direct_reference
scope:
  - api
  - docker
  - scripts/node/dev-up
  - extension bus
---

# 后端模块边界不自动升级为 HTTP 服务边界

## 规则

- 前期后端部署默认保持一个业务后端容器和一个对外端口，内部通过 Rust crate、trait/port 与启动装配形成模块化单体。
- 插件总线描述扩展点、贡献、顺序、作用域、生命周期和交付语义，不承载传输协议选择；模块组合不等于 HTTP 通信。
- 第三方 RuntimeExtension 可执行代码仍应由宿主通过受控子进程、stdio 等边界隔离；这类执行隔离不要求把插件宿主本身部署成常驻第二 HTTP 服务。
- 只有出现独立扩缩容、故障域、安全域或跨主机调度等已验证需求时，才重新评估独立服务与网络协议。

## 原因

用户澄清插件体系前期原意是单后端容器内的模块分离。当前同时存在 `api-server` 直接链接 `plugin-runner`、进程内执行插件宿主和 7801 独立 HTTP 服务，混淆了 crate、进程与容器三种边界，并扩大编译、部署和运行态耦合。

## 适用场景

后端依赖架构重整、插件总线演进、`plugin-runner` 去服务化、容器合并、Cargo 编译增量优化与运行时隔离设计。
