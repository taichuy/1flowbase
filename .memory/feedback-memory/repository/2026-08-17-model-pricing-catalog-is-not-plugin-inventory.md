---
memory_type: feedback
feedback_category: repository
topic: 厂家计费目录不得从插件模型清单生成
summary: 厂家计费是独立业务目录，不是插件或 RuntimeExtension 模型 inventory；只采用厂家公开的标准 API USD 价格，排除 Coding Plan、Credits 和订阅套餐；默认零价只能由单条 zero/any 全局兜底表达。
keywords:
  - model-pricing
  - provider-pricing
  - runtime-extension
  - zero-any
  - fallback
  - standard-api-price
match_when:
  - 生成或同步厂家模型计费目录
  - 修改官方计费 catalog 校验
  - 设计缺少精确计费规则时的兜底
created_at: 2026-08-17 20
updated_at: 2026-08-18 11
last_verified_at: 2026-08-18 11
decision_policy: direct_reference
scope:
  - api/apps/api-server/assets/model-pricing
  - ../1flowbase-official-plugins/model-pricing
  - api/crates/control-plane/src/billing.rs
---

# 厂家计费目录不是插件 inventory

## Rule

厂家计费目录必须独立维护，不能扫描 RuntimeExtension / 插件模型清单并为每个模型生成计费规则。官方默认零价只使用一条 `provider_code=zero`、`upstream_model_id=any` 的全局兜底；运行时先匹配精确 `provider/model`，再匹配该兜底。

目录价格只录入厂家公开发布的标准 API USD Token 单价；Coding Plan、Credits、订阅套餐或未正式开放模型不属于该目录真值。

## Reason

厂家、计费规则和插件是不同 owner。按插件模型生成大量零价记录会把插件 inventory 误当计费真值，造成重复规则、错误归属和页面噪声。

## Applies When

- 发布、导入或升级厂家计费 catalog。
- 修改计费规则匹配、缓存失效或缺规则拒绝逻辑。
- 测试官方目录是否覆盖插件模型。
