# Scope

- 本目录是 adapter-facing 稳定控制面契约的唯一 owner。
- 只放 repository trait、contract error、持久化输入输出、纯投影和可独立验证的确定性函数。
- 不依赖 `control-plane`，不放 service、路由、数据库查询或宿主装配。
- contract 变更必须保持 DTO / 领域字段语义一致，并由 contract authenticity test 覆盖。

## Evidence

- `cargo test -p control-plane-contracts` 必须通过；adapter consumers 必须完成 test-target compilation。
- dependency boundary test 必须证明本 crate 不反向依赖 `control-plane`。

## Resources And Stop

- 只接受现有 adapter 所需的最小依赖闭包，不为方便吸收 service helper。
- 若 contract 必须调用 application / orchestration 实现、复制同一 DTO 或改变外部语义，停止并返回 Root 重构边界。
