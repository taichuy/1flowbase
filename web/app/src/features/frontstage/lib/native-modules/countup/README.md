# CountUp native module

- 新增直接依赖 `react-countup@6.5.3`（MIT），传递依赖 `countup.js@2.10.1`（MIT）；用于统计数字动画，复用宿主 React。
- 通过 registry 按需加载默认组件和 `useCountUp`；编辑器声明来自安装包，不开放任意 npm import。
- 接入层归一化 CommonJS 默认导出，兼容 Vite dev 的 exports 对象与生产 / 测试加载器的已解包组件。
- Block 位于 ShadowRoot，Hook 应传 React ref / 元素引用，不使用 document 范围的字符串 ID 查询。
- 原 Demo 保持不变。自定义 Statistic formatter 时，小数位由 CountUp 的 `decimals` 控制。
