# Provider routing boundary

Provider routing is owned by `orchestration-runtime`. A compiled invocation freezes the candidate order and readiness, then selects through the built-in rule or the typed `provider_distribution_rule/v1` Runtime operation. RuntimeExtensionHost may execute a plugin decision, but Orchestration validates the returned target against that frozen snapshot before invoking any Provider.

The three built-ins share the same receipt shape: `none` selects the first eligible target for a new session and reuses its durable original owner on later requests, `round_robin` consumes one shared counter value per invocation and pins retries, and `retry_round_robin` uses the invocation-local attempt index. Plugin rules cannot choose retry count, call Providers, inspect prompts/secrets, or mutate candidate state.


The control-plane invoker seals a routing scope from authenticated workspace/application/API key/actor,
flow, explicit session identity, node, original routing pool, provider and model. The scope excludes
candidate order and distribution-rule revisions. Headerless calls without a conversation signal do
not acquire a shared session. The durable repository stores only the scope digest and instance ID.

For `none`, an existing owner is resolved before main-instance readiness/eligibility filtering; its
original route error propagates. Capability preflight cannot replace that owner. New selections bind
atomically after semantic preflight; concurrent losers use the persisted winner only if it remains
in their eligible snapshot, otherwise they fail. Explicit rules keep their own decisions while
first-write recording the original owner. Native opaque continuation ownership and recovery budgets
remain independent constraints and cannot be bypassed by a distribution decision.
