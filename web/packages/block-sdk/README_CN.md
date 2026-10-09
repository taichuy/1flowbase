# React 区块多语言

[English](README.md)

宿主会向页面画布和 JSX Studio 运行面板中的原生 React 区块提供 `ctx.i18n`，
按应用当前语言加载后端翻译目录。区块无需自行请求目录或导入翻译库。

```tsx
import type { BlockComponentProps } from '@1flowbase/block-sdk';

export default function Block({ ctx }: BlockComponentProps) {
  return <span>{ctx.i18n.t('Account')}</span>;
}
```

- `t(key)` 使用后端目录的全局唯一 key，按完整字符串查询。当前官方 key 是英文原文，
  例如 `Account`；句点和冒号属于 key 本身。管理台前端资源中的 `auto.*` 是另一套目录。
- `t(key, { defaultValue, values })` 支持默认文案和 `{{variable}}` 插值，React 会转义展示文本。
- key 不存在时返回 `defaultValue`，未提供则返回 key。目录加载中或请求失败时也采用该回退。
- `locale` 与 `ctx.ui.locale` 一致；`status` 为 `loading`、`ready` 或 `error`。
- 切换语言会更新上下文并重新渲染组件，保留组件本地 Hook 状态。请在渲染时调用 `t`，
  不要把翻译结果固定在模块作用域或初始 state 中。
- 现有目录接口要求 `i18n.catalog.view` 权限和根工作区；其他调用方无法取得目录，
  宿主会呈现 `error` 状态。本能力不扩展后端权限。

```tsx
ctx.i18n.t('Hello {{name}}', {
  values: { name: 'Ada' },
  defaultValue: 'Hello {{name}}'
});
```

目录快照按用户、工作区、权限和语言缓存五分钟，窗口恢复焦点时遵循查询的正常重新验证。
目录编辑尚无推送更新。不经过页面宿主而独立构建的运行面板上下文会呈现 `error`，仅提供默认文案回退。
