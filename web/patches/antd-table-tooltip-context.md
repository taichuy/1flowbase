# Ant Design Table and Tooltip context

AntD 6.7.1 uses one context in Table's measurement row, its filter dropdown,
and Tooltip. Move the context to `_util` so importing Tooltip does not import
from Table. The context default, provider, consumers and behavior are unchanged.
The anonymous visual path still rejects AntD entirely, and the deferred style
provider still rejects all Table modules.

Both ESM and CommonJS are patched. On upgrade, remove this patch if Tooltip no
longer imports from `table/`, then verify the production sign-in boundary.
