---
memory_type: feedback
feedback_category: repository
topic: Provider 认证语义与协议翻译归插件所有
summary: 宿主只提供通用插件配置与 secret 的安全存储、注入和必要的通用操作桥；OAuth、token refresh、模型发现、供应商协议翻译及传输策略必须由对应 Provider 插件拥有。
keywords:
  - provider auth
  - oauth
  - token refresh
  - secret storage
  - provider plugin boundary
  - ChatGPT Codex
  - AI Native translation
match_when:
  - 设计带 OAuth 登录和 refresh token rotation 的 Provider 插件
  - 判断认证流程、模型发现、供应商协议翻译应放在宿主还是插件
  - 为 Provider 插件增加通用 auth operation 或 secret patch contract
created_at: 2026-08-19 18
updated_at: 2026-08-19 18
last_verified_at: 2026-08-19 18
decision_policy: direct_reference
scope:
  - api/crates/plugin-framework
  - api/apps/plugin-runner
  - api/crates/control-plane/src/model_provider.rs
  - ../1flowbase-official-plugins/runtime-extensions/model-providers
---

# Provider 认证语义与协议翻译归插件所有

## 时间

`2026-08-19 18`

## 规则

- Provider 插件拥有供应商专属 OAuth authorize/exchange/refresh 语义、token 解析和轮换、模型发现、AI Native 到供应商 wire 的翻译、SSE/WebSocket 状态机，以及供应商网络和身份头适配。
- 宿主只拥有通用插件配置/secret 的安全持久化、运行时注入和必要的通用调用桥，不解析或硬编码任何 ChatGPT 专属 URL、scope、token 字段或协议事件。
- 若现有插件 contract 不能完成网页登录或 rotated secret 回写，应增加最小的通用 Provider auth operation / secret patch capability；该 bridge 只传递 typed operation 与受 schema 约束的 patch，不接管供应商认证生命周期。

## 原因

用户纠正过：“宿主只是存储插件配置而已，翻译还是 AI Native 的活”。把通用 secret storage 推导成宿主拥有 OAuth 生命周期，会让 ChatGPT 私有语义泄漏到 control plane，并破坏 Provider 插件作为供应商适配 owner 的边界。

## 适用场景

- 新增 ChatGPT subscription / Codex OAuth Provider。
- 设计 Provider 登录按钮、OAuth callback、refresh token rotation 和 secret persistence。
- 评估 Provider host 是否需要新增 auth method 或插件结果回写能力。

## 备注

Hosted tools 是否由上游执行仍按具体 Provider contract 判断；宿主透传工具声明与事件不等于获得工具执行所有权。
