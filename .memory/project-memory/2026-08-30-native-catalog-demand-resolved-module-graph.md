---
memory_type: project
topic: Native Runtime Catalog 端到端按需模块图
summary: 用户批准以加载放大率、失败影响半径和关键路径规模约束 Native Catalog；#1953 为 icons、dayjs 与 dnd-kit 建立 dev demand resolver 和 production lazy chunk 双路径。
keywords:
  - frontstage
  - native runtime
  - Vite
  - demand resolver
  - module graph
  - optimizeDeps
created_at: 2026-08-30 16
updated_at: 2026-08-30 16
last_verified_at: 2026-08-30 16
decision_policy: verify_before_decision
status: user-acceptance
scope:
  - issue:1953
  - web/app/build/native-demand-resolved-modules.ts
  - web/app/build/native-ant-design-icons-modules.ts
  - web/app/build/native-dayjs-modules.ts
  - web/app/build/native-dnd-kit-modules.ts
  - web/app/vite.config.ts
---

# 当前决策

Native Catalog 的控制面可以登记全部已安装模块，但 Vite 开发数据面不得因此预优化或传输全部 leaf。开发环境以精确 `Map` virtual resolver、`Set` allowlist、Promise single-flight 和包内等价 ESM distribution 按 demand 加载；production 保留静态 lazy imports，让 Rollup 发现全部可用 chunk，但首屏只预加载共享关键路径。

# 指标与边界

- 加载放大率 `A = 实际请求 leaf / 页面使用唯一 leaf`，目标接近 1。
- 失败影响半径 `B = 单模块失败影响 Block 数`，依赖 #1950 Runtime Cell 合同保持 1。
- 关键路径规模 `C` 不随 Catalog 总条目增长；dev loader 的动态 import 边为常数。
- `@ant-design/icons`、`dayjs/*` 与 `@dnd-kit/*` 的已开放范围不缩减；dev 使用上游包自身 ESM 分发，production 使用原始请求入口。
- 普通 bundleless dev 的全应用源码请求量与 FRPC 首次加载继续属于 #1946，不由 #1953 建设第二套远程入口。

# 当前证据

定向 Vitest 5 files / 44 tests 与 Runtime Cell 17 tests 通过；相关 ESLint、diff check、TypeScript 和 production Vite build 通过。冷开发请求代表 icon、dayjs plugin 与 dnd-kit root 时没有新增 optimizer discovery/reload。受保护页面快照最终 ready、0 failed resources；首次 bundleless 冷导航仍可能超过 15 秒，保留给 #1946。
