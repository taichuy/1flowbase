# Scope

- 本目录是模型用量报告的既有页面 / workflow authoring 工具，不拥有平台请求日志写入或通用报告 API。
- `range.js` 归一化 SQL 时间边界；`report.sql` 拥有聚合语义；JSX 只消费报告结果，保留未知用量与币种隔离。

## Evidence Lanes

- `_tests/*.test.js` 只放可在普通 Node tooling lane 执行的纯函数与源码契约测试，不读取私有 `.env` 或要求已发布页面。
- PostgreSQL 服务集成测试放 `integration/`，通过显式入口运行；只消费显式数据库 URL，使用事务内临时表并回滚。线上入口归属 `repo-backend-test-storage-postgres-1-of-4`，其失败必须进入同一候选门禁报告。
- 已发布部署验收放 `acceptance/`，要求显式 API、workspace、页面、block、时间范围与凭据；复用 `page-debug` 临时 session owner 并在 `finally` 回收。
- SQL 算法通过不代表已发布 API / 页面通过；缺少对应运行证据时保留未验证结论，不以跳过或固定本地部署替代。
