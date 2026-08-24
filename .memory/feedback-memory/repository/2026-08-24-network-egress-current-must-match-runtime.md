---
memory_type: feedback
feedback_category: repository
topic: Network egress current version must match runtime artifact
summary: 代理实例只绑定稳定插件家族，不绑定版本 installation；界面 current 版本必须与实际启动版本一致。
keywords:
  - network-egress
  - current version
  - installation_id
  - runtime worker
  - version switch
created_at: 2026-08-24 17
updated_at: 2026-08-24 17
last_verified_at: 2026-08-24 17
decision_policy: direct_reference
scope:
  - api
  - web/app/src/features/settings/network-center
---

# 代理插件 current 版本必须等于实际运行版本

## 规则

网络中心安装或切换代理插件版本后，后端投影的 current 版本、既有代理实例解析到的 artifact、实际启动的 versioned worker 必须一致。不得把“current”解释为仅供新建入口使用；需要保留旧版本时，只能作为可回滚制品，不得继续成为未显式固定的运行真值。

代理实例的配置、secret 和生命周期属于 provider instance；插件版本属于家族的部署/激活状态。不得通过升级时批量改写实例 `installation_id` 来维持两者一致；应由运行节点使用稳定家族身份解析 current artifact。worker 必须按 provider instance 隔离，不同订阅不得因处于同一插件版本而共用 worker/secret。

## 原因

用户看到 `0.2.8` 已安装且被选为 current，会合理预期后续代理调用运行 `0.2.8`。界面与进程版本分裂会让安全、资源和功能修复静默失效，且当前 UI 没有表达实例级版本固定语义。

## 适用场景

- 代理插件安装、升级、降级和版本切换。
- 代理供应方实例绑定、历史数据迁移、worker 加载、drain/unload 与回滚。
- Network Center 版本 DTO 与 UI 展示。
