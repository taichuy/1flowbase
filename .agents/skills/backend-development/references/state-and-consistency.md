# State And Consistency Checklist

## Define The State First

写实现前先回答：

- 状态集合是什么？
- 允许哪些流转？
- 每个动作能把状态从哪里推到哪里？
- 哪个入口负责改这个状态？
- 展示态是否只有一个事实锚点？例如同一运行态不能让 cache、snapshot、latest run、last-run panel 分别代表不同真值。

## Review Questions

- 谁能写这个状态？
- 是不是只有一个明确入口？
- 有没有绕过主入口的隐式改写？
- 数据一致性和状态一致性分别靠什么保证？
- 失败、重试、回滚时状态是否仍然可解释？
- 如果存在 read model、cache、snapshot、latest 查询和详情查询，它们是否能追溯到同一个 `run_id / scope_id / state_version`？

## Resource Action Kernel Checks

- 新 Core 写动作是否注册为 resource/action，还是仍由 route 直接调散落 service
- route、worker、HostExtension route 是否都通过 action dispatch 进入同一写入口
- hook / policy / validator 是否只在显式 pipeline 中运行，且顺序可解释
- HostExtension 是否只写 extension-owned sidecar table，或通过 Core action 改变 Core 真值
- audit、outbox、幂等和事务边界是否属于 action owner，而不是插件私自补

## Warning Signs

- 多个 handler 都能直接把对象改成“完成”
- repository 内偷偷附带状态跳转
- 只有代码路径，没有显式状态规则
- 没有办法回答“为什么现在是这个状态”
- 一个面板读 snapshot，另一个面板读 latest record，用户却以为它们代表同一次运行

## 涉及用户、作用域或业务文件时

- 删除用户清理登录身份、角色绑定与凭据，保留业务资源、会话与账务历史。历史归属 UUID 可指向已删除用户；查询不以用户存在为条件，姓名 / 账号可空。
- session 持有 `tenant_id`、`current_workspace_id`，登录结果和请求中间件传递后者，一次请求对应一个 workspace。建模 `scope_kind` 为 `workspace` 或 `system`，物理路由列用 `scope_id`，system 用 `SYSTEM_SCOPE_ID`。应用标识是 `application_id`，不另设 team/app 别名。
- 业务二进制归 `storage-object`，不同于插件包；默认本地根是 `api/storage`，`rustfs` 非默认。`root/system` 管 `file_storages` 及绑定，workspace 创建和消费可见 `file_tables`。记录保留上传时的 `storage_id`，改绑只影响新上传。
