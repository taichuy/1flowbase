---
memory_type: project
topic: API 宿主 jemalloc THP 防膨胀、流式校验、启动快照单次收敛与插件多版本收口已验证同步
summary: 基于 dev 平衡方向完成 THP 防膨胀与子进程隔离、64KiB 流式哈希、按作用域单次启动快照刷新及插件多版本退役收口四项修复，克隆库 A/B 验证就绪耗时降 35.8%、启动峰值 VmHWM 降 179.5 MiB、幽灵运行态清零，已同步至本地与 taichuy-dev 的 dev 工作区。
keywords:
  - memory
  - jemalloc
  - thp
  - plugin-version-reconciliation
  - startup-snapshot
match_when:
  - 排查 api-server 容器或开发环境启动内存占用、AnonHugePages 膨胀与启动耗时
  - 修改插件多版本切换、启动对账 reconcile_all_installations 或受管扩展启动快照刷新逻辑
  - 准备将本轮内存与插件状态收口修复发版部署至 iqlater-ali 或其他 Docker 环境
created_at: 2026-11-27 13
updated_at: 2026-11-27 13
last_verified_at: 2026-11-27 13
decision_policy: verify_before_decision
scope:
  - docker
  - api/apps/api-server
  - api/crates/control-plane
  - api/crates/runtime-extension-host
  - api/crates/plugin-framework
  - api/crates/storage/durable/postgres
---

# API 宿主 jemalloc THP 防膨胀、流式校验、启动快照单次收敛与插件多版本收口已验证同步

## 标题

完成 `api-server` 启动内存防膨胀与插件多版本退役收口四项闭环并同步 `dev`

## 时间

`2026-11-27 13`

## 谁在做什么

开发者与 AI 协同 Codex 在 `taichuy-dev` 隔离 worktree（基于 `b39fb8065c67c429513b7efb7b326d925412c5cf`）完成了宿主内存与插件生命周期四项核心修复及克隆库 A/B 实测，并将补丁同步至远程 `/home/taichuy/git/1flowbase` 与本地 `E:\code\taichuCode\1flowbase` 工作区（保留线上 `iqlater-ali` 只读未动，待后续正常发版更新镜像）。

## 为什么这样做

采用平衡方向一次性治理分配器页粒度、大插件整包读入、启动期按安装记录重复构建快照以及历史插件版本退而不休四类叠加根因：
1. 容器与 Compose 为宿主配置 `_RJEM_MALLOC_CONF="thp:never,background_thread:true"`（支持 `API_JEMALLOC_CONF` 覆盖），并在受管插件 worker 子进程拉起处剥离该变量，不硬编码 `narenas` / `dirty_decay_ms`，也不使用传染子进程的 `prctl`。
2. 插件可执行文件与归档 `payload_sha256` 改为 `64 KiB` 固定块流式校验（兼容归一化排序与 framing 字节语义）。
3. `refresh_boot_snapshots` 改为按去重作用域单次批量准备并原子发布快照与事件目录。
4. 版本切换与启动对账仅在跨全部工作区零绑定、零活跃实例/依赖引用时将旧版本收口为 `disabled`，重置当前节点未激活记录的幽灵 `runtime_status = active`，同时完整保留 `installed_versions` 列表与 `switch_version` 回滚能力。

## 为什么要做

线上 `iqlater-ali`（内核 `thp=always` 下 `AnonHugePages` 达 `404 MiB`）与 `taichuy-dev` 开发环境（277 条历史插件记录全量陪跑、单次百兆读包与重复快照构建）出现启动内存飙升至 400–560 MiB 及旧版本幽灵运行态残留问题。克隆库同机 A/B 实测表明：就绪耗时由 `203.86s` 降至 `130.89s`（`-35.8%`），峰值 `VmHWM` 由 `752.45 MiB` 降至 `572.99 MiB`（`-179.46 MiB`），稳态中位 RSS 下降 `51.67 MiB`，当前节点 `disabled + active` 幽灵记录由 `39` 清零至 `0`。

## 截止日期

无硬性外部截止日期；代码已在本地与 `taichuy-dev` 就绪，随下一次容器镜像构建与部署即可在 `iqlater-ali` 生效。

## 决策背后动机

遵循“功能完整性 > 稳定性 > 性能 > 可演进性”与不引入新第三方依赖原则：既消除 THP 2 MiB 大页驻留放大与百兆级瞬时缓冲峰值，又通过跨工作区引用与锁顺序不变量保护多工作区共用旧版本、排空平滑退役与历史版本回滚可用性。

## 关联文档

- `tmp/test-governance/ab-startup-benchmark/ab_summary.json`
- `docker/README.md`
