# Infinite Scroll

- 新增直接依赖：`react-infinite-scroll-component@7.2.1`（MIT），用于触底加载、反向滚动及可选下拉刷新；无额外运行时依赖，复用宿主 React。
- 低代码按需开放默认 `InfiniteScroll` 组件；类型声明来自已安装包。包的 `useInfiniteScroll` hook 暂未注册。
- `scrollableTarget` 字符串在当前 Block 的 ShadowRoot 内、父 DOM 提交后解析，不跨 Block 查找同名 ID；缺失目标报错，不静默退回 window。HTMLElement 目标直接传递。
- 未指定目标时使用 Block 的 scroll owner；指定 `height` 时沿用组件自身滚动容器。观察、加载保护及监听清理复用上游实现。
- 数据请求、分页、异常重试和虚拟列表仍由使用者管理。此次未改写数据库 Demo，也未改变其示例数据源或 50 条展示条件。
