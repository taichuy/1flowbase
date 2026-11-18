# Block Design Controls

底座提供 `@1flowbase/block-design`，供编译后的区块代码复用设计态入口与悬浮工具。模块注册、编辑器类型和 Shadow DOM 样式由宿主加载，无需区块自行注入样式或引入依赖。

```tsx
import { BlockDesignButton, BlockDesignToolbar, BlockDesignMenu } from '@1flowbase/block-design';

<BlockDesignToolbar
  disabled={!designMode}
  tools={
    <BlockDesignMenu menu={{ items }}>
      <BlockDesignButton aria-label="字段设置" />
    </BlockDesignMenu>
  }
>
  {fieldTitle}
</BlockDesignToolbar>

<BlockDesignButton variant="initializer">配置字段</BlockDesignButton>
```

- `BlockDesignToolbar` 统一悬浮边框、右上角工具条、键盘焦点与触屏入口；透传 DOM 事件和 ref，可作为 Popover 触发器。
- `BLOCK_DESIGN_THEME` 可传给 Ant Design `ConfigProvider.theme`，用于统一设计态表单、筛选器与菜单的蓝色反馈；表格与表单区块使用同一份主题。
- `BlockDesignButton` 的默认设置按钮复用现有区块的 `FrontstageNodeActionButton`（14px 按钮、8px 图标）。`initializer` 保留齿轮图标与可见文字，作为“配置字段／配置操作”等常驻显式入口，不用孤立加号替代文字指引。
- 显式配置按钮与表格动作按本地 NocoBase 实测统一：14px 常规字重、28px 高、左右 11px 内边距、4px 图标间距、6px 圆角。运行态保留品牌色，设计态沿用编辑蓝色。
- `BlockDesignMenu` 使用与区块更多操作相同的菜单密度、边框、圆角和阴影，默认单击打开。
- 设计态由调用方提供。运行态隐藏配置控件；数据权限与配置保存仍由宿主和后端负责。
- 颜色沿用 `design-mode-theme.ts` 的蓝色编辑语义；不复制参考软件的品牌色。

## Block Settings Registration

区块整体配置通过 `useRegisterBlockSettings` 注册到当前画布区块的外层配置菜单，区块内部只渲染设置弹窗，不再创建第二个整体设置按钮。注册作用域由每个画布 slot 单独拥有，卸载时回收；数据保存与权限仍属于原宿主 contract。

```tsx
import { useRegisterBlockSettings } from '@1flowbase/block-design';

useRegisterBlockSettings([
  {
    id: 'table:layout',
    label: '高度、分页与表格大小',
    onSelect: () => setDialog('layout')
  }
]);
```

字段、行操作等局部配置继续使用就近的悬浮入口。配置菜单字号为 11px、边缘留白 8px，避免把整体设置与局部操作混成两个区块菜单。
