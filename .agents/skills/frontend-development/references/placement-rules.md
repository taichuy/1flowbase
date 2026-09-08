# Frontend Placement Rules

## Purpose

当前规则只回答一个问题：这段前端代码该放哪一层。

先按职责落点，再谈抽象和复用。不要为了“看起来更整洁”把页面、壳层、路由、请求和工具重新揉在一起。

## Directory Roles

| 目录 | 放什么 | 不放什么 |
| --- | --- | --- |
| `app/` | 应用启动、Provider 组装、入口级装配 | 页面业务、route truth layer、请求消费 |
| `app-shell/` | 共享壳层、顶栏、导航容器、账户菜单 | 业务页面、route tree、feature 私有组件 |
| `routes/` | `route id / path / selected state` 与权限消费 / guard 配置 | 后端授权策略、页面 JSX、壳层样式、请求逻辑 |
| `features/*/pages` | 页面容器、页面级状态、页面级组合 | 可复用通用组件、底层请求 client |
| `features/*/components` | 当前 feature 私有组件 | 过早共享组件、路由真值层 |
| `features/*/api` | 当前 feature 的 query key、queryFn、mutation、请求适配 | 通用 transport、全站共享 client |
| `features/*/hooks` | 当前 feature 的交互和组合 hooks | 跨 feature 工具、全局状态 |
| `features/*/lib` | 当前 feature 内部 helper、mapper、view model | 跨 feature 纯工具、全局共享请求 |
| `features/*/schema` | 当前 feature 的 schema adapter、fragment、renderer registry | 通用 schema UI 基础组件 |
| `features/*/store` | 当前 feature 私有客户端状态 | 跨页面共享状态、服务端数据缓存 |
| `shared/ui` | 多个 feature 真实复用的稳定组件 | 壳层专属组件、单 feature 组件 |
| `shared/utils` | 纯函数工具、格式化、解析、轻量 helper | 请求、副作用、全局状态依赖 |
| `shared/api` | 多个 feature 共享的请求编排 | 单 feature 请求消费 |
| `web/packages/api-client` | 底层 API client、DTO、transport、base URL | 页面状态、React Query 消费逻辑 |

## Placement Order

遇到一段代码时，按这个顺序判断：

1. 这是壳层还是业务？
2. 这是路由真值层还是页面展示？
3. 这是底层请求还是 feature 消费？
4. 这是纯函数还是 feature 私有逻辑？
5. 这段代码是否已经被多个 feature 真实复用？

只要前四步还没判断清楚，就不要先抽成 `shared/*`。

## Promotion Rules

### Component Promotion

- 默认先放 `features/*/components`
- 满足以下条件后，才提升到 `shared/ui`
  - 已被两个以上 feature 真实复用
  - 组件语义稳定，不依赖某个 feature 的业务名词
  - 不携带当前 feature 的路由、状态或请求上下文

### API Promotion

- 默认先放 `features/*/api`
- 常规消费链是 `api-client -> features/*/api -> UI`；`shared/api` 是共享编排的可选 owner，不是必经层，也不反向依赖某个 feature 的私有请求实现。
- 满足以下条件后，才提升到 `shared/api`
  - 两个以上 feature 共享同一套请求编排
  - 不只是共享一个底层 endpoint，而是共享消费方式

### Utility Promotion

- 默认先放 `features/*/lib`
- 满足以下条件后，才提升到 `shared/utils`
  - 是纯函数
  - 不依赖 feature 业务上下文
  - 有明确跨 feature 复用场景

## Smell Checks

出现以下信号时，基本说明落点错了：

- 页面文件里直接写请求函数
- `router.tsx` 同时承载菜单、route tree、账户菜单、页面组合
- `shared/ui` 里出现 `embedded app`、`agent flow` 这类业务词
- `shared/utils` 里依赖 `window`、store、network 请求或 feature 状态
- `shared/api` 只有一个 feature 在用
- section 页面绕过 `shared/ui/section-page-layout` 自造侧栏、移动端 tabs 或权限分流
- 为了“以后可能复用”提前把东西提到 `shared/*`

## Minimal Examples

### Good

```text
web/packages/api-client/src/index.ts
  fetchApiHealth()

web/app/src/features/home/api/health.ts
  homeHealthQueryOptions()

web/app/src/features/home/pages/HomePage.tsx
  useQuery(homeHealthQueryOptions())
```

底层请求、feature 消费、页面展示三层分开。

### Bad

```text
web/app/src/features/home/HomePage.tsx
  fetch(...)
  queryKey = ...
  mapper(...)
  render(...)
```

请求、适配、渲染混写，后续很难拆。
