# 执行阶段与扩展计划

[总览](README.md) · 上一篇：[契约与身份](02-contracts-and-identity.md) · 下一篇：[终态与交付](04-finalization-and-delivery.md)

## 主路径与分支

```mermaid
flowchart TB
    R["Resolve / PrincipalEstablished"] --> Z["Core Authorization → ordered extension veto"]<br/>    Z --> A["Core Admission → ordered extension veto"]<br/>    A --> B["typed Before"]<br/>    B --> D["Dispatch: 冻结 Attempt / Target"]<br/>    D --> H["exactly-one typed Handler"]<br/>    H -->|成功| S["After"]<br/>    H -->|失败| F["Failure"]
    Z -->|拒绝| F
    A -->|拒绝| F
    B -->|拒绝或失败| F
    S --> C["适用 Completion"]
    F --> C
    D -->|取消或超时| C
    C --> T["Terminal Receipt"]
```

图示主要分支；取消可在其他受控等待阶段发生，按实际到达阶段执行适用收尾。宿主已经完成的凭证认证不在 Kernel 重复执行；Kernel 确认可信主体和认证配置。未解析入口与认证拒绝见[认证边界](02-contracts-and-identity.md)。

## 阶段 owner

| 阶段 | 责任 |
| --- | --- |
| Received | 协议解析、大小限制与 correlation |
| Resolved / PrincipalEstablished | 验证绑定、冻结计划并确认可信主体 |
| Authorized | 业务 owner 判定调用资格，扩展只能进一步否决 |
| Admitted | 业务 owner 判定额度、状态、容量等准入条件 |
| Prepared | typed Before 校验与获准的有限输入修改 |
| Dispatched / Executing | 冻结目标后执行业务，事务仍由业务 owner 持有 |
| PostProcessed / Completion | 观察主结果与收尾，不改写已提交业务事实 |
| Terminal / Projected | Kernel 记录终态；适配器另行投影协议 |

Core deny 不可被 extension allow 恢复；拒绝后不运行 Handler。Definition、Decision、Hook、Handler 通过真实注册编译为可执行计划，不能用手写 fingerprint 代替绑定。

## 扩展坐标与开放边界

接口层坐标包括 definition、authentication_adapter、authorization、admission、before、handler、after、failure、completion。节点存在不等于 HostExtension、RuntimeExtension、CapabilityPlugin 都获得同样权限。

```text
合法空计划             → 正常核心生命周期
已绑定且适用的计划     → 按冻结顺序执行，或记录明确未执行原因
声明存在但实现不合法   → 在装配边界拒绝，不能静默降级为空计划
```

插件的空间坐标是目标 Interface、point/phase、scope、权限和隔离；时间坐标是 version、Graph/Registry、artifact/generation、Invocation/Attempt。每项实际开放贡献需声明 typed contract、ordering、可见事实、mutation/failure/delivery 语义和身份。

接口管理可以用受控 typed registration 验证执行契约。三级插件开放则需真实 declaration → loader/activation → graph/registry → invocation，覆盖依赖、冲突、停用、版本切换和在途隔离。native HostExtension 的 restart-scoped 管理不能被快照测试解释成 Rust 热卸载。

源码与局部规则：[interface-runtime/AGENTS.md](../../../api/crates/interface-runtime/AGENTS.md)。
