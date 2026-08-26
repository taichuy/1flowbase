# Scope

- 本目录是 adapter-facing 稳定控制面契约的唯一 owner。
- 只放 repository trait、contract error、持久化输入输出、纯投影和可独立验证的确定性函数。
- 不依赖 `control-plane`，不放 service、路由、数据库查询或宿主装配。
- contract 变更必须保持 DTO / 领域字段语义一致，并由 contract authenticity test 覆盖。
