# 已验证前端模块

宿主根据已授权的插件贡献目录传入 `code_modules`，不从用户组件源码推导插件权限。组件记录只是可发现的代码资源，不是模块执行授权。

- `binding: host`：仅引用宿主已注册模块与声明的导出，不携带资产。
- `binding: fetched`：一个 `browser_module`、可选 `shadow_style`；所有执行和样式字节先验证 manifest 中的 SHA-256，再加载。
- 原有自包含 ESM 可以直接导出声明的成员。
- 需要复用宿主 React、Ant Design 等依赖的 bundle 可导出 `createFrontendModule(hostModules)`。参数只包含同一 `code_modules` 清单中显式声明的 host source/export，值保持宿主引用身份。factory 返回对象或其 Promise，返回对象必须包含所有声明导出。

```js
export function createFrontendModule(hostModules) {
  const React = hostModules.react;
  return {
    Example: () => React.createElement('span', null, 'Example')
  };
}
```

构建工具可以把 CommonJS bundle 包在 factory 内，并把 `require` 限制为 `hostModules` 自有键。不要把第二份 React 打进 bundle，不依赖全局模块变量、外部 CDN 或页面 import map。

调用方拥有 loader epoch：身份、工作区、目录版本或授权变化时废弃旧 loader，取消请求并释放 Blob URL；新 epoch 重新使用后端授权目录和资产接口。前台 artifact cache 的 module policy identity 包含完整模块声明，升级不复用旧版本工厂。已经执行的 trusted-host JavaScript 不是安全沙箱，停用不等于撤回浏览器已执行的任意副作用；后端继续拒绝新的未授权资产读取。

[English](./README.en.md)

前台会保留每个 fetched module 的安装归属和自身 host export 清单。不同插件引用同一宿主模块的不同合法子集不会冲突；一个 factory 不会收到其他插件的依赖声明。同一 fetched source 的跨安装归属或字节/版本/依赖冲突仍然拒绝。
