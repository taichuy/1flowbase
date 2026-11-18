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
