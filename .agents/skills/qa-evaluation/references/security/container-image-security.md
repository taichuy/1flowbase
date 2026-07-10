# Container Image Security QA

## Purpose

用于 QA 阶段评估容器镜像扫描结果、Dockerfile 基础镜像风险、Trivy / GHCR 报告和镜像发布门禁。目标是给出可复查的影响判断，不把基础镜像漏洞维护扩大成项目长期安全平台工作。

## Load When

- 用户要求跑或解释容器镜像扫描、Trivy、GHCR、Dockerfile、镜像发布安全门禁。
- QA 报告中出现 `container-image-security.json`、`trivy-*.json`、`container-images` scope。
- 需要判断 HIGH / CRITICAL 漏洞是否应阻断发布、是否应公开到 GitHub Issue、是否需要专项修复。

## Default Policy

- 容器漏洞扫描默认是 `warning/report`，不是发布阻断。
- HIGH / CRITICAL 发现要保留报告和证据，但不要默认创建公开 GitHub Issue。
- PUBLIC 仓库中不要把完整 CVE 清单、受影响包版本、镜像引用和攻击面判断发布到 Issue 正文；可写脱敏摘要和 artifact 路径。
- 基础镜像 / OS package 漏洞优先通过拉取最新稳定官方镜像并重建处理。
- 不为 `unfixed`、不可触达、仅基础镜像传导的漏洞长期手工维护发行版包，除非它进入重点修复条件。

## Evidence Checklist

评估前至少收集这些证据，缺失项要在 QA 结论中标为未验证：

- 镜像：组件名、image ref、tag、digest 或 Actions run id。
- 来源：最终 runtime Dockerfile stage 的 base image、安装的 OS packages、是否多阶段构建。
- 扫描：Trivy JSON 路径、HIGH / CRITICAL 数量、fixed / unfixed 数量、主要包名。
- 配置：nginx / runtime / plugin / backend 是否实际加载受影响模块或库。
- 入口：公网入口、HTTP、WebSocket、上传、插件安装、后端解析、数据库访问路径。
- 权限：容器用户、挂载、secret、数据库凭据、网络暴露、是否 privileged。

## Risk Triage

先判断“可触达性”，再判断“影响等级”。不要只因为镜像里存在受影响库就说当前服务可被利用。

### Promote To Blocking Or High

满足任一项时，升级为重点修复建议；如果在发布链路中命中公网入口或用户数据风险，可建议阻断：

- 可通过公网 HTTP / WebSocket / 静态资源处理 / 反向代理路径直接触发。
- 当前配置实际加载受影响模块，例如 nginx XSLT 模块、image filter、动态脚本模块等。
- 漏洞影响是 RCE、容器逃逸、权限提升、任意文件读取、secret/token 泄露。
- 可读取或修改数据库、用户数据、插件包、workspace 内容或系统配置。
- 攻击者可提交触发输入，例如 XML、archive、plugin package、uploaded file、template、script。
- 镜像以高权限运行、带敏感挂载、含数据库凭据，且漏洞可跨越普通进程崩溃影响边界。

### Keep As Warning

满足这些条件时通常保持 warning，不建议投入专项修复：

- 漏洞来自基础镜像或发行版 OS package，项目只间接受影响。
- 受影响库存在于镜像中，但当前服务配置没有加载它，也没有把用户输入传给它。
- 漏洞是 DoS，但只影响不可触达的模块或本地辅助工具。
- Trivy 标记 `unfixed`，发行版尚无修复包。
- 有 fixed version，但通过官方镜像重建或常规 base image refresh 即可处理。
- 内部镜像不直接对外暴露，风险主要由网络边界和运行时权限控制。

## Remediation Order

1. 重建镜像，确保拉取最新稳定官方 base image。
2. 如果 fixed package 已发布且 base image 未更新，可在 runtime stage 做最小 OS package upgrade。
3. 删除不需要的 runtime package 或模块，例如未使用的 nginx dynamic module。
4. 如果漏洞可触达且影响高，才考虑更换 base image、distroless/scratch/wolfi、改运行时权限或重构处理链路。
5. `unfixed` 漏洞记录影响面和等待上游修复；不要伪造本地兼容补丁。

## Report Shape

QA 输出必须区分事实、推断和建议：

```text
容器扫描摘要：
- scope/run/artifact:
- 组件：web/api-server/plugin-runner
- HIGH/CRITICAL:
- fixed/unfixed:

影响判断：
- 可触达路径：
- 当前配置是否加载受影响模块：
- 用户数据 / token / 数据库风险：
- 结论：Blocking / High / Medium / Low warning / 未验证

建议：
- 立即修复：
- 跟随官方镜像重建：
- 暂不处理原因：
- 需要补充证据：
```

## Examples

- `nginx:alpine` 内存在 `libxml2` HIGH，当前 nginx 配置只是静态文件和反向代理，未加载 XSLT/XML 处理模块：记录 warning，建议跟随官方镜像升级；不要说普通 HTTP 请求可直接利用。
- `debian:bookworm-slim` runtime 中 `libgnutls30` 有 fixed version：优先重建或最小 OS package upgrade；若服务没有 TLS 终止或 GnuTLS 调用路径，保持 warning。
- 插件上传链路解析 archive / XML / script，且受影响库实际处理用户上传内容：升级为重点修复，检查是否能读取文件、执行代码或泄露 token。

## Stop Conditions

- 没有 Trivy JSON、Actions artifact、Dockerfile 或运行配置证据时，不下确定安全结论。
- 公开仓库中不要发布完整漏洞明细到 Issue；需要共享时优先使用脱敏摘要和短期 artifact。
- 用户要求修复漏洞时，先确认是否属于“可触达且影响高”的路径；否则默认给出官方镜像跟随升级建议。
