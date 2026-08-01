---
memory_type: feedback
feedback_category: repository
topic: 本地扩展产物存在时不得自动远程修复
summary: `api/plugins` 已有本地产物时必须以本地内容为准，即使签名或 checksum 异常也只告警，不得自动拉取远端备份、修复或替换，因为本地内容可能正在用于开发调试。
keywords:
  - extension-center
  - api/plugins
  - local-first
  - signature
  - checksum
  - development
match_when:
  - 设计扩展安装、验签、reconcile 或启动恢复
  - 处理本地产物签名或 checksum 异常
  - 判断是否自动下载远端产物覆盖本地
created_at: 2026-08-01 08
updated_at: 2026-08-01 08
last_verified_at: 2026-08-01 08
decision_policy: direct_reference
scope:
  - api/plugins
  - extension center
  - plugin bootstrap
  - plugin reconcile
---

# 本地扩展产物存在时不得自动远程修复

## 时间

`2026-08-01 08`

## 规则

- `api/plugins` 已存在目标产物时，以本地产物为唯一依据。
- 签名缺失、未知密钥、验签失败或 checksum 不一致只产生状态、警告和审计，不触发远端下载。
- 不自动拉取远端备份，不自动修复，不自动覆盖或回滚本地文件。
- 只有本地目标产物缺失，并且既定 bootstrap 或用户显式安装 / 更新动作要求获取时，才允许访问远端。

## 原因

- 本地文件可能是开发者正在调试的修改版本，远端恢复会破坏开发现场。
- 自动恢复增加隐式网络副作用和第二份真值，违背本地文件是已安装唯一真值的边界。

## 适用场景

- 扩展中心安装、更新与版本检测
- 插件启动加载与本地 inventory reconcile
- 签名、checksum 与可信度诊断
- Docker / 源码开发环境的默认扩展初始化
