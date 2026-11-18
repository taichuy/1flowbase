# api/：从入口找到行为的 owner

```text
apps/api-server（装配 Router、Catalog、OpenAPI、进程内 RuntimeExtensionHost）
  → crates/api/ 协议适配域 → interface-runtime 冻结计划 → typed Handler → 业务 / 执行 / 存储
```

这是请求路径，不是 Cargo 依赖方向。宿主是唯一 composition root；新行为放到负责它的模块，宿主只做装配。

| 改动落点 | 职责与下一站 |
| --- | --- |
| `apps/api-server/` | boot、workers、bin 和实际装配。宿主入口、认证或流式收尾改动见[调用生命周期](../docs/architecture/interface-lifecycle.md) |
| `crates/api/` | HTTP Kernel、请求中间件与协议适配域；[域目录](crates/api/AGENTS.md)区分真实实现和宿主 facade |
| `crates/` | 按[crate 地图](crates/AGENTS.md)选业务、执行或存储 owner 和允许依赖，再看目标 crate 的局部 AGENTS |
| `plugins/` | 插件源码、包、安装态与模板见[工作区说明](plugins/README.md)；宿主贡献与受管执行的边界见[插件组合](../docs/architecture/plugin-composition.md) |
| `storage/`、`target/` | 默认本地业务文件、Cargo 构建产物，均不是源码落点 |

模块级测试放 owner 的 `src/_tests`；宿主启动、完整 HTTP 或跨 crate 行为放集成目标。在 [API 测试目标表](apps/api-server/tests/README.md) 按行为找已有 target：`--test <target>` 选择编译范围，测试名过滤只选择运行范围。

实现时按 [backend-development](../.agents/skills/backend-development/SKILL.md) 的主题索引取对应约定，例如 Settings operation、事务或网络出口。
