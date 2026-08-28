# Provider routing boundary

Provider routing is owned by `orchestration-runtime`. A compiled invocation freezes the candidate order and readiness, then selects through the built-in rule or the typed `provider_distribution_rule/v1` Runtime operation. RuntimeExtensionHost may execute a plugin decision, but Orchestration validates the returned target against that frozen snapshot before invoking any Provider.

The three built-ins share the same receipt shape: `none` pins the first eligible target, `round_robin` consumes one shared counter value per invocation and pins retries, and `retry_round_robin` uses the invocation-local attempt index. Plugin rules cannot choose retry count, call Providers, inspect prompts/secrets, or mutate candidate state.
