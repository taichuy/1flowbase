---
memory_type: feedback
feedback_category: interaction
topic: 后台表格操作按钮归属 Ant Table title
summary: Settings 表格页的筛选卡片使用左 label 右控件、每行最多三项和右下角重置/筛选；表格级操作进入表格自身的 title / toolbar 行。
keywords:
  - settings
  - data table
  - table title
  - toolbar
  - actions
match_when:
  - 后台管理页要求在表头增加按钮行
  - 参考图同时标注筛选区按钮和表格表头上方空白区
  - 需要区分筛选工具栏与表格级操作栏
  - Settings 列表页的顶部筛选条件横向平铺显得拥挤或难看
created_at: 2026-07-31 14
updated_at: 2026-07-31 16
last_verified_at: 2026-07-31 16
decision_policy: direct_reference
scope:
  - web/app/src/shared/ui/data-table
  - web/app/src/features/settings
---

# 后台表格操作按钮归属 Ant Table Title

## 时间

`2026-07-31 14`

## 规则

- “表头增加按钮行”默认指表格自身的 `title / toolbar` 行；导出、刷新、字段配置等表格级动作放在这里并右对齐。
- 筛选卡片只承载筛选、搜索、排序条件，不通过内部换行伪造表格操作栏。
- 筛选项采用左侧 label、右侧控件；桌面一行最多三项，超出换行，小屏按两列 / 单列诚实降级。
- 筛选卡片右下角统一放“重置 / 筛选”，条件先作为草稿编辑，点击“筛选”后再统一提交。
- 参考图同时圈出原按钮位置和表格表头空白区时，先按空间归属判断移动目标，再实现。
- 表格已经提供“查看 / 编辑”等明确行级按钮时，行内空白区域不再隐式触发查看或编辑，也不显示手型光标；每个动作只由对应按钮进入。

## 原因

- 只在筛选卡片内部拆成两行不会改变筛选区与表格的结构归属，视觉分割仍然明显。
- 表格级动作进入 Ant Table 原生 `title` 后，动作与被操作的数据主体形成直接关系。
- 显式按钮与整行隐藏点击并存会造成重复入口和误触，尤其会让用户无法预判点击空白区域是查看还是编辑。

## 适用场景

- `/settings/*` 后台列表管理页
- `DataTable` 的导出、刷新、字段配置、批量操作入口
