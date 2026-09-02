---
memory_type: project
topic: 远程 Vite 开发链路的 bundle profile
summary: 用户批准 #1946 采用 Vite Full Bundle Mode 优先、稳定 bundle fallback；实测 Full Bundle 命中停止条件，当前 dev:remote 使用带 source map 的 production bundle + preview，公网请求数下降 96.4%，首次加载时间仍待用户决定是否接受。
keywords:
  - Vite
  - Full Bundle Mode
  - FRPC
  - Nginx
  - remote debug
  - performance
created_at: 2026-08-30 00
updated_at: 2026-08-30 00
last_verified_at: 2026-08-30 00
decision_policy: verify_before_decision
status: implementation
scope:
  - issue:1946
  - web/app/package.json
  - web/app/vite.config.ts
  - web/app/src/app/_tests/vite-config.test.ts
---

# 当前决策

- 用户批准 Single Issue [#1946](https://github.com/taichuy/1flowbase/issues/1946)：默认本机 `dev` 保持 bundleless/HMR；远程链路优先试验 Vite 8 Full Bundle Mode，命中插件兼容、频繁 reload、状态丢失或性能停止条件时切换稳定 bundle fallback。
- Vite `--experimentalBundle` 本地可启动，但公网出现大量 Ant Design icons `/@vite/lazy` 动态导入失败；未修改 Native Runtime 或 Vite 上游来兼容实验模式。
- 当前候选实现中 `dev:remote` 使用 `remote-debug` production bundle + preview 并生成 source map；`dev:remote:experimental` 仅保留给后续 Vite 升级复验。

# 证据与未结算边界

- 原 bundleless 公网无缓存加载约 1421 个模块请求、约 180 秒；HTML TTFB 约 0.2 秒。
- 稳定 fallback 公网 page-debug 进入 ready，页面错误、控制台错误和 warning 均为 0；资源记录为 51 个，请求数下降约 96.4%。
- 公网完整快照约 23～25 秒，本机约 4.6 秒，尚未达到 Issue 中“不超过本机约 2 倍”的时间口径；主要剩余约束是 gzip 后约 3.3MB 关键 JS 经 FRPC 的吞吐。
- #1946 保持 `phase:implementation`；用户体验后需要决定接受首次加载边界，还是新增 Nginx 云端 asset cache / 首屏模块图瘦身范围。未达到 AC 前不提交或推送候选改动。
