---
memory_type: project
topic: main 合并后模型供应商契约与门禁复评
summary: 用户在 `2026-04-20 15` 合并 `1flowbase` 最新 `main` 后重新复评。最新主仓提交 `4b8d63e5 fix: align settings model provider api responses` 已修复 settings 对 `/api/console/model-providers/catalog` 的主消费链硬不兼容：`api-client` 新增 wrapper response，settings wrapper 改为读取 `response.entries`。但 `style-boundary` 的 settings 场景仍保留旧数组 mock，已被定向 vitest 复现为失败；同时仓库内仍没有独立的 `test-contracts/verify-contracts` 门禁，官方插件跨仓库校验也仍是单向。
keywords:
  - model-provider
  - contract
  - settings
  - style-boundary
  - verify-repo
  - verify-ci
  - official-plugins
match_when:
  - 需要继续评估模型供应商共享契约门禁
  - 需要判断 latest main 是否已修复 catalog 不兼容
  - 需要回看 style-boundary settings 场景为何在 main 后失败
  - 需要决定 contract gate 与跨仓库 gate 的优先级
created_at: 2026-04-20 15
updated_at: 2026-04-20 15
last_verified_at: 2026-04-20 15
decision_policy: verify_before_decision
scope:
  - /home/taichu/git/1flowbase/web/packages/api-client/src/console/modeling/model-providers.ts
  - /home/taichu/git/1flowbase/web/app/src/features/settings/api/model-providers.ts
  - /home/taichu/git/1flowbase/web/app/src/style-boundary/registry.tsx
  - /home/taichu/git/1flowbase/scripts/node/verify-repo.js
  - /home/taichu/git/1flowbase/scripts/node/verify-ci.js
  - /home/taichu/git/1flowbase-official-plugins/.github/workflows/provider-ci.yml
---

# main 合并后模型供应商契约与门禁复评

## 时间

`2026-04-20 15`

## 已验证事实

1. `catalog` 主消费链已从“数组”改成“wrapper response -> entries”。
2. 定向运行 `web/app` vitest 时，`settings-api.test.ts` 与 `model-providers-page.test.tsx` 通过，但 `style-boundary/_tests/registry.test.tsx` 因旧 `/catalog` mock 失败。
3. `verify-repo` 与 `verify-ci` 入口未新增独立 contract gate。
4. `1flowbase-official-plugins` 的 `provider-ci` 仍然是官方插件仓库单向检出主仓并调用主仓脚本，主仓没有反向触发插件仓库校验。

## 当前判断

- 旧结论里“`/catalog` 已在 latest main 上硬不兼容”不再成立。
- 新的阻塞项变成“主链修了，但 full gate 相关替身没有全部同步”，尤其是 style-boundary settings 场景。
- 质量治理层面的核心缺口没有变：缺少共享契约独立门禁，缺少变更感知的跨仓库反向门禁。
