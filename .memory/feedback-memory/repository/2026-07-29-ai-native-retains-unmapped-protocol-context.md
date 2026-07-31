---
memory_type: feedback
feedback_category: repository
topic: AI Native 保留未映射协议上下文，Provider 选择性消费
summary: 入口协议中无法映射的额外参数不应丢失或阻断请求；经入口安全过滤后的协议上下文由 Start 作为受 ProtocolContextEnvelope 格式约束的普通变量发布给下游，允许用户读取、修改或替换，Provider adapter 只发送能够诚实表达的字段。
keywords:
  - ai native
  - protocol context
  - residual fields
  - provider adapter
  - cross protocol
  - passthrough
  - start variable
  - downstream authoring
  - typed contract
  - schema validation
match_when:
  - 设计映射协议到 AI Native 再到 Provider 的跨协议翻译
  - 处理客户端未知字段、协议扩展或供应商不支持的可选参数
  - 判断未知参数应拒绝、丢弃、保留还是透传
  - 设计 Start 节点协议上下文变量及下游读取、修改或替换行为
created_at: 2026-07-29 11
updated_at: 2026-07-31 23
last_verified_at: 2026-07-31 23
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
- 经入口安全过滤后的协议上下文由 Start 作为普通 JSON 变量发布给下游；其可见性、选择、节点传递和用户改写语义与普通变量一致，不应只作为 LLM 私有运行时旁路或不可编辑 locator 展示。
- 工作流节点可以读取、修改或替换协议上下文；用户改出不满足消费契约的值时，由实际消费节点按普通变量契约明确失败，不由 Start 阻止作者修改。
- “普通变量”描述可见性、流转和可修改性，不表示接受任意 JSON。协议上下文必须具有稳定的 `ProtocolContextEnvelope` 格式；变量选择器只接受声明为该契约的上游输出，编译 / 发布阻止不兼容来源，动态值仍在运行时按同一契约校验并显式失败，不做静默 fallback。
- Provider adapter 只把能够诚实表达、且目标 Provider 接受的字段渲染到上游请求；普通未消费字段留在 Native，不需要发送，也不需要报错。
- 不能把所有入站 HTTP header/body 字段盲目复制到另一个 Provider。认证、hop-by-hop、签名和敏感字段必须保持原有安全边界；残余字段也不能覆盖 Typed AI Native 的已知语义真值。
- 对工具结果、continuation 等省略后会改变操作含义的必要语义，不能当普通可选扩展静默丢弃；应明确失败。可选提示或未知扩展则允许保留并省略。

## 原因

AI Native 是完整承载层，工作流是用户可控的数据处理层，Provider adapter 是选择性出站层。入口安全过滤负责阻断凭据和危险传输字段进入工作流；过滤后的协议上下文一旦进入工作流，就应服从普通变量语义。这样既不会因客户端协议扩展而频繁阻断，也不会因为跨供应商盲目透传未知字段造成严格 schema 拒绝、凭据泄漏或语义覆盖。

## 适用场景

- Anthropic ingress 转 OpenAI / Responses Provider
- OpenAI Chat、Responses、Anthropic 三种公开协议互转
- Claude Code、Codex、OpenCode 携带客户端专属扩展
- `sys.protocol_context`、Provider request renderer 与跨协议 capability policy
