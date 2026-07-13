---
memory_type: feedback
feedback_category: repository
topic: SettingsFeature API ownership 跟随设置用例而非底层资源
summary: 用户纠正 SettingsFeature 授权边界：角色获得某设置项权限后，应能完成该设置页提供的全部操作；读取其他领域数据不应要求额外 business action 或另一个 feature。
keywords:
  - settings-feature
  - api-ownership
  - role-grant
  - console-settings
  - use-case
match_when:
  - 为 SettingsFeature 归属 API route
  - 设置页读取共享领域数据
  - 判断是否需要额外 business action 或其他 feature
created_at: 2026-07-13 23
updated_at: 2026-07-13 23
last_verified_at: 2026-07-13 23
decision_policy: direct_reference
scope:
  - api
  - web/app/src/features/settings
  - https://github.com/taichuy/1flowbase/issues/1256
---

# SettingsFeature API ownership 跟随设置用例

## 规则

- 角色是授权持久化单位；用户只通过绑定角色获得有效权限，不新增用户直授权限。
- 角色授权某个 SettingsFeature 后，可以进入该设置项并完成页面提供的整组操作。
- API ownership 按设置用例归属，不按底层数据资源名归属。成员页为分配角色读取 role options，仍属于成员设置能力。
- 共享数据继续复用后端 service / repository；不得因此要求第二个 SettingsFeature、business action、`AnyFeature` 或运行时权限展开。
- `/api/console` 旧权限 contract 未稳定时，可以直接重命名、拆分和删除旧接口，不保留 fallback。

## 原因

若按底层资源归属 API，设置页每读取一个共享资源就会要求额外权限，重新制造本 Issue 要删除的 action 矩阵和多重授权心智。

## 适用场景

- 成员设置读取角色选项并保存成员角色绑定。
- 任一 Settings 页面复用其他领域的查询或写入能力。
- Core / HostExtension 注册 Settings API scope 与 compiled inventory。
