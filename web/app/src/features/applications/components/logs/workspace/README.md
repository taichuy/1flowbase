# Application Logs

应用路由与 Frontstage 使用同一个 `ApplicationLogsWorkspace`，业务查询与详情加载仍归 applications feature。原路由保留原应用 scope 与表格偏好；总日志只查询当前可访问的 Agent Flow 应用，使用独立表格偏好。初始显示标题、应用、请求模型、推理强度、协议、状态、费用、总 tokens、缓存命中率、开始时间十列及操作列；其他字段仍可在列设置中选择，已保存的个人偏好优先。

Frontstage native React module `@1flowbase/application-run-logs` 导出：

- `ApplicationLogs`：传入 `applicationId`，展示单应用的完整日志页面。
- `AllAgentFlowLogs`：展示全部 Agent Flow 应用的完整日志页面。
- `ApplicationRunLogViewer`：保留既有单运行查看器契约。

页级组件支持 `sizing={ctx.ui.sizing}`，报告页面期望高度并使用宿主分配的内容尺寸，避免自动高度在移动端裁切分页。详情、轨迹、时间线浮窗使用宿主 overlay；样式只注入当前区块的 Shadow DOM。

```jsx
import React from 'react';
import { AllAgentFlowLogs } from '@1flowbase/application-run-logs';

export default function AllApplicationLogsBlock({ ctx }) {
  return <AllAgentFlowLogs sizing={ctx.ui.sizing} />;
}
```

总日志的批量导出按 `application_id` 分组下载归档；导入需确认目标应用，并保留该应用的导入任务恢复信息。后端负责跨应用筛选、排序及分页；前端不合并各应用的分页结果。
