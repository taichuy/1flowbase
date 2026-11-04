# Scope

- 仅拥有每调用同步 QuickJS、strict plugin / legacy Code 双 codec、console、interrupt/cancel 与 exe 版本根内 verified script loader。
- 不依赖任何内部 crate；不吸收计划、状态、node/schema/payload、协议 context、Host Registry 或业务贡献。
- 每调用新 Runtime/Context；只可共享已验证的不可变 source，不能共享 live JS 状态。
- 默认 strict，不调用 guest toJSON/getter/可改写序列化 intrinsic；legacy 只保持既有 Code 的 JSON.stringify 和错误语义，不做 strict fallback。
- memory/stack 为 0 时跳过 setter，保留引擎默认；timeout/output 来自调用者，不添加猜测的插件私有预算。
- 公共 API 不暴露 Path/PathBuf。PackagedScript.path 只允许 exe 版本根内的逻辑 selector；expected version/hash 来自 compiled release descriptor，不来自请求。
- 安装版本目录在加载期间不可变；canonical containment、bounded regular UTF-8、版本/hash 校验必须同时成立。
- cancel 不等于退出或 join；载体 owner 必须 stop 后等待真实执行回收。SDK 常驻 worker 生命周期不在本 crate 扩展。

# Evidence

- 模块测试放 src/_tests；覆盖严格正反例、legacy 回归、隔离、预算/恢复、取消及 relocation/version/hash/path escape。
- 不为局部验证启动服务或重型全仓 Cargo；集中 QA 由 Root 在冻结 assembly 执行。
