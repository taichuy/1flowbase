---
memory_type: feedback
feedback_category: repository
topic: 模型供应商连接 schema 必须以后端 projection 为真值
summary: 修复模型供应商连接配置缺字段时，不要在前端按 base_url/api_key 推断兜底；#626 后的历史安装兼容应通过后端 catalog projection backfill 或 refresh 处理。
keywords:
  - model-provider
  - catalog-projection
  - form_schema
  - frontend-fallback
  - backend-truth
created_at: 2026-06-04 00
updated_at: 2026-06-04 00
decision_policy: direct_reference
scope:
  - web/app/src/features/settings/components/model-providers
  - api/crates/control-plane/src/plugin_management
---

# 模型供应商连接 schema 必须以后端 projection 为真值

## Rule

模型供应商连接配置字段缺失时，前端不得自行推断 `base_url` / `api_key` 或维护第二套兼容逻辑；必须由后端 catalog projection 提供完整 `form_schema`。

LLM 节点中仅对某供应商有意义的调用选项，也必须由该供应商注册并随 catalog projection 下发；宿主前端只依据声明渲染，不按 `provider_code` 硬编码字段或传输语义。

## Reason

插件本身声明了 `config_schema`，空 `form_schema` 是 #626 之后历史安装缺少 `plugin_package_catalog_projection` 的数据兼容问题。前端兜底会掩盖后端投影缺失，并造成字段契约双源。

## Applies When

- `settings/model-providers` 抽屉连接配置缺字段。
- catalog response 中 `catalog_refresh_status` 为 `missing` 或 `failed`。
- 修改 model provider catalog、plugin scan、install/reconcile、projection refresh 相关逻辑。
- 新增 OpenAI 等供应商专属的 LLM 节点调用选项。
