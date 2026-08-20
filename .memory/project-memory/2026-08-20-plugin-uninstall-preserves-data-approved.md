---
decision_policy: verify_before_decision
date: 2026-08-20 18
status: approved
---

# Plugin uninstall preserves durable data

The user approved GitHub Issue #1785 as a `grade:g3` Single Issue for a new development conversation. Plugin uninstall means quiescing and disposing the plugin runtime, removing local artifacts, and marking the plugin unavailable; installation identities, instances, encrypted configuration, assignments, workflow references, imported data, and history remain intact.

The implementation must reuse the Extension Bus and runtime lifecycle foundation delivered by #1688 rather than create another plugin kernel. Existing destructive family/version deletion semantics are not compatibility requirements. Reinstalling the same stable plugin identity should reuse the retained installation and restore its configuration.

This work is needed because the model-provider catalog exposes no uninstall action and current backend family deletion removes provider instances and installation records. No external deadline was set; the user will implement it in a separate conversation and return for review.

## Scope resolution

On 2026-08-20, the user limited #1785 to `RuntimeExtension` and `CapabilityPlugin`.
Native `HostExtension` remains trusted, boot-time activated and restart-scoped; it is not
part of this issue's hot-unload contract. The complete `control-plane` and `api-server`
library suites were not green during the concentrated QA batch, so their failures remain
explicitly unverified rather than being attributed to or masked by this issue.
