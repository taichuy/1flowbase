---
memory_type: feedback
feedback_category: repository
topic: AI Native 保留未映射协议上下文，Provider 选择性消费
summary: 入口协议中无法映射的额外参数不应丢失或阻断请求；它们继续保留在 AI Native 协议上下文中，具体 Provider adapter 只发送能够诚实表达的字段，普通未消费字段留在 Native 而不盲目跨供应商透传。
keywords:
  - ai native
  - protocol context
  - residual fields
  - provider adapter
  - cross protocol
  - passthrough
match_when:
  - 设计映射协议到 AI Native 再到 Provider 的跨协议翻译
  - 处理客户端未知字段、协议扩展或供应商不支持的可选参数
  - 判断未知参数应拒绝、丢弃、保留还是透传
created_at: 2026-07-29 11
updated_at: 2026-07-29 11
last_verified_at: 2026-07-29 11
decision_policy: direct_reference
scope:
  - api/crates/control-plane/src/application_public_api
  - api/crates/plugin-framework
  - api/crates/orchestration-runtime
  - ../1flowbase-official-plugins/runtime-extensions/model-providers
---

# AI Native 保留未映射协议上下文，Provider 选择性消费

## 规则

- 入口 mapper 把已经理解的语义映射为 Typed AI Native，同时把剩余的合格协议字段保留在协议上下文中；普通未知字段不因无法映射而让整个请求失败。
- 工作流中的协议上下文可以继续流转，并按既定产品语义由节点读取、修改或替换。
- Provider adapter 只把能够诚实表达、且目标 Provider 接受的字段渲染到上游请求；普通未消费字段留在 Native，不需要发送，也不需要报错。
- 不能把所有入站 HTTP header/body 字段盲目复制到另一个 Provider。认证、hop-by-hop、签名和敏感字段必须保持原有安全边界；残余字段也不能覆盖 Typed AI Native 的已知语义真值。
- 对工具结果、continuation 等省略后会改变操作含义的必要语义，不能当普通可选扩展静默丢弃；应明确失败。可选提示或未知扩展则允许保留并省略。

## 原因

AI Native 是完整承载层，Provider adapter 是选择性出站层。这样既不会因客户端协议扩展而频繁阻断，也不会因为跨供应商盲目透传未知字段造成严格 schema 拒绝、凭据泄漏或语义覆盖。

## 适用场景

- Anthropic ingress 转 OpenAI / Responses Provider
- OpenAI Chat、Responses、Anthropic 三种公开协议互转
- Claude Code、Codex、OpenCode 携带客户端专属扩展
- `sys.protocol_context`、Provider request renderer 与跨协议 capability policy
