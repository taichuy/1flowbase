# web/：页面、状态与请求放哪里

页面先从 `app/src/routes/` 找到 feature，再沿 `pages → api → packages/api-client` 找到消费的后端 DTO。视觉与交互以 [DESIGN.md](../DESIGN.md) 为准。

| `app/src/` 下的目录 | 职责 |
| --- | --- |
| `app/`、`app-shell/`、`routes/` | 依次负责启动与 Provider、共享壳层与菜单、route id / path / 选中态与权限 guard；授权真值在后端 |
| `features/*/` | `pages` 放页面容器，`components` 放局部组件，`api` 放 query / mutation，`hooks`、`store` 放本功能交互和草稿，`schema` 放私有 renderer / adapter，`lib` 放内部工具 |
| `shared/`、`state/` | 跨 feature 复用的 UI、纯函数和请求编排分别进 `ui`、`utils`、`api`；跨页面客户端状态进 `state`，服务端数据留在 query 层 |
| `styles/`、`style-boundary/` | 前者放 token、reset、全局样式；页面样式跟组件。后者放共享样式的场景注册与回归 |
| `test/`、`_tests/` | 前者只放全局 setup / 跨场景工具；业务用例跟最近 owner。模型供应商页面按 `features/settings/_tests/model-providers-page/` 场景收纳 |

`packages/api-client` 拥有 transport 与原始 DTO；新增模块按平面或 feature 收纳。`packages/ui` 放设计系统组件；独立 runtime、protocol、schema 或 SDK 才单列 package。

开发路径、图表安全边界、i18n 和页面取证入口见 [frontend-development](../.agents/skills/frontend-development/SKILL.md)。
