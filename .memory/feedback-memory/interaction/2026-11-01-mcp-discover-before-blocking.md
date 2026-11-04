---
memory_type: feedback
feedback_category: interaction
topic: mcp-discover-before-blocking
summary: 用户指出 1flowbase 无现成 Tool 时应继续使用后台接口发现与接口转 Tool 能力，而非立即宣告无法执行。
created_at: 2026-11-01 16
updated_at: 2026-11-01 22
decision_policy: direct_reference
---

## 规则
用户要求通过 1flowbase MCP 完成业务操作时，没有现成 Tool 不等于无法完成。继续核对后台设置接口、interface 的 bindable 与契约，并在已授权任务边界内复用或转换、挂载和验证 Tool。

## 原因
2026-11-01 用户纠正了仅因没有角色 Tool 就停止的判断；本轮实际转换并调用角色查询与创建均成功。

## 适用场景
已有 MCP 接口发现与 Tool 配置能力、但缺少业务 Tool 的任务。真实阻断必须落实到具体接口的 disabled_reason、权限拒绝或执行错误；不能把目录缺失当作授权不足。

## 底座修复与验收补充
- 用户已明确要求修复 MCP 权限底座时，不以“接口不可绑定”、只建空角色或补配置作为最终交付；定位并修复 contract、schema 与真实授权消费链。
- 验收须覆盖同一 MCP 的权限发现与配置、受限测试用户的允许/拒绝及自提权负例、真实 Chrome 页面和数据库一致性；root 成功、HTTP 200 或保存成功不能替代受限用户证据。用户已授权的临时账号/凭据用于验证后回收。
- 原因：2026-11-01 用户指出底座有问题，明确要求修复并经浏览器、数据库和测试用户验证无误后再交付。适用于该类已明确授权的底座修复，不扩展到未授权产品语义变更。
