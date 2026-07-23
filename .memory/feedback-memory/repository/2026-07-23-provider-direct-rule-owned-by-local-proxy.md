---
memory_type: feedback
feedback_category: repository
topic: Provider 直连例外归本地代理规则管理
summary: 本机通过 Clash/Mihomo 统一代理时，特定 provider 上游需要直连应优先在 7897 代理的 profile rules enhancement 中配置 DIRECT，不在 Backend NO_PROXY 中长期打补丁。
keywords:
  - provider
  - Clash Verge
  - Mihomo
  - DIRECT
  - NO_PROXY
  - 7897
match_when:
  - Provider 自定义 Base URL 经本机代理被 WAF 或 Cloudflare 拒绝
  - 用户要求特定域名直连且本机使用 Clash/Mihomo 统一代理
created_at: 2026-07-23 07
updated_at: 2026-07-23 07
last_verified_at: 2026-07-23 07
decision_policy: direct_reference
scope:
  - local development proxy configuration
  - Clash Verge profile rule enhancement
  - provider runtime networking
---

# Provider 直连例外归本地代理规则管理

## 规则

本机通过 Clash/Mihomo 的 7897 端口统一承接代理时，provider 未配置插件级代理但目标域名需要直连，优先在当前 Clash profile 绑定的 rules enhancement 中增加精确 `DOMAIN,<host>,DIRECT` 规则。不要把 Backend `NO_PROXY` 当作持久方案，也不要硬编码项目仓库或 provider 插件。

## 原因

用户明确要求直连策略由本地代理文件统一管理。这样应用仍经过标准代理入口，Clash 负责按规则选择 DIRECT，订阅更新时 rules enhancement 仍可复用，且不会让不同 Backend 启动方式产生不一致环境。

## 适用场景

全局 `HTTP_PROXY/HTTPS_PROXY` 指向 Clash/Mihomo，代理出口被目标 WAF 拒绝，而普通代理请求与 `--noproxy` 对照证明直连可达时。
