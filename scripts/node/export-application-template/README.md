# 应用模板导出更新

从运行中的应用导出定义到官方插件仓分类目录。页面、应用、数据建模定义关系、MCP 的依赖闭包由后端计算；不包含业务记录、账号密码和外部连接凭据。

首次保存选择，例如 `selection.json`：

```json
{
  "page_ids": ["页面 UUID"],
  "application_ids": ["应用 UUID"],
  "data_model_ids": [],
  "mcp_instance_ids": ["1flowbase"]
}
```

在主仓执行：

```sh
node scripts/node/export-application-template/cli.js \
  --target /home/taichuy/git/1flowbase-official-plugins/applications-demo/@taichuy/gateway-demo \
  --selection /absolute/selection.json \
  --name 'Gateway Demo' \
  --api-base-url http://127.0.0.1:7800
```

首次命令在目标目录保存 `export.config.json`（资源选择、来源地址、名称、稳定模板身份）。后续只运行：

```sh
node scripts/node/export-application-template/cli.js \
  --target /home/taichuy/git/1flowbase-official-plugins/applications-demo/@taichuy/gateway-demo
```

可再次传 `--selection` 调整选择；资源内容或模板名称/说明变化时自动递增版本，否则维持版本和导出时间。脚本通过项目临时 owner session 访问 API，finally 回收，凭据来自主仓私有 `.env`，不写入模板。隔离工作树可指定 `--repo-root /home/taichuy/git/1flowbase` 使用来源开发仓配置。

导出先写临时目录并校验所有文件，再原子替换；失败保留原模板。源码目录按页面、应用、数据模型、MCP 和插件依赖分类，`manifest.json` 只存资源引用和文件摘要。`export.config.json`、README、目录元数据不进入发布 ZIP。提交这些源码变更并推送插件仓 main 后，applications-demo 发布 workflow 签名 ZIP、上传不可变 Release、刷新分页和搜索目录。脚本本身不自动提交或推送。

纯源码校验/本地打包由插件仓 `scripts/application-template/archive.mjs` 提供，与发布和 Docker 打包共用。安装时后端验证归档，再将资源交给同一个安装入口。系统界面 `/settings/backups` 也可创建或上传 ZIP。
