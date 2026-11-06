# MCP return controls and native content projection

[中文](return-controls.md)

Tool `max_inline_chars` sets the serialized JSON text budget (4,000 characters by default). `response_fields` and `string_ranges` select metadata fields and Unicode character ranges; cached detail can be read through `mcp_result` without repeating the business operation. See the Chinese companion for the full control contract.

Managed operations may declare `mcp.output_mapping`:

```json
{
  "mcp_content": [{"type": "image", "data_path": "/data", "mime_type_path": "/mime_type"}],
  "structured_content_path": "/metadata"
}
```

Paths are explicit JSON Pointers into the business response. A block's paths must be valid, distinct, non-overlapping, and separate from the metadata object. The host extracts declared content without inferring it from stdout. Missing paths, non-object metadata, malformed mappings or invalid native fields fail explicitly. The existing image declaration and business response wire format remain supported.

| `type` | Required paths | Optional paths |
| --- | --- | --- |
| `text` | `text_path` | `annotations_path` |
| `image`, `audio` | `data_path`, `mime_type_path` | `annotations_path` |
| `resource_link` | `uri_path`, `name_path` | `mime_type_path`, `description_path`, `annotations_path` |
| `resource` | `uri_path`, exactly one of `text_path` / `blob_path` | `mime_type_path`, `annotations_path` |

Image and audio data must be nonempty canonical base64, with valid `image/*` or `audio/*` MIME types respectively; there is no business MIME allowlist. Empty resource blobs are valid files. Resource URIs must be absolute; the host neither downloads nor resolves them. Embedded resources emit `{type:"resource",resource:{uri,mimeType?,text|blob}}`. Legal annotations are retained. `resource_read` defaults to false and requires at least one resource declaration (optionally skip_if_null), with optional resource links; initial output can be link-only, but successful rereads must return exactly one matching embedded body; mappings with text, image or audio cannot enable it; it declares eligibility for the resource-read adapter.

Locally mapped native content, including text, is retained independently of metadata selection, text pagination and continuation storage. `max_inline_chars`, field/range selection and cached detail apply only to the separate metadata object. A low metadata budget never truncates a native block. Upstream proxy text keeps its existing metadata-budget behavior, while non-text content survives paging.

The final 256 KiB JSON-RPC envelope limit exempts only validated `image.data`, `audio.data`, embedded `resource.resource.blob`, and `resources/read` result `contents[].blob`. Validation uses a fixed decode buffer without a second full blob allocation. MIME types, URIs, annotations, resource text, metadata, unknown fields and JSON framing all remain budgeted. Invalid binary fields receive no exemption.

The business `output_schema` remains unchanged. `mcp_get.result_schema` projects the actual declared metadata subschema; an unresolved schema remains unconstrained rather than inventing fields. Omitting `mcp.output_mapping` preserves existing text projection.

## Resource rereads

The server advertises `capabilities.resources = {}` alongside tools and supports this tools/resources subset for `2025-11-25` and `2025-03-26`. Initialize honors either supported proposal and returns `2025-11-25` for unknown proposals. Subsequent stateless HTTP requests specify `MCP-Protocol-Version`; absent headers default to `2025-03-26`, unsupported headers receive HTTP 400. Legacy wire converts resource links to JSON text preserving URI/name and structuredContent to JSON text (without repeating equivalent existing JSON text), omitting these unsupported legacy fields/blocks. Modern wire preserves all five native content types. Subscriptions, standard Tasks and 2026 extensions are not advertised.

For an InterfaceWrapper mapping with `resource_read:true`, all declared embedded resources and resource links, including independent link-only output, receive a host URI, `flowbase-resource://read/<BASE64URL_JSON>`. General external links without resource_read remain unchanged and are never fetched automatically. The self-contained versioned reference contains user/workspace, selected instance, tool ID, current `des_id`, caller business arguments and original resource URI. It stores no response binary, request headers, session credentials or hidden/default bindings and requires no URI registry or binary cache. This declaration is for file reads whose caller arguments are safe to expose as locators.

`resources/read` accepts `{uri}` and returns `{contents:[{uri,mimeType?,text|blob}]}`. The requested host URI remains the returned URI, without recursive wrapping. A reference is a locator, not authorization: each read checks the current user/workspace, selected and visible tool/instance, explicit opt-in and current revision before dispatch. Revoked visibility, disabled opt-in, changed revision, malformed or unknown host URI fail without calling a tool. Editing locator arguments does not grant additional access.

The RPC uses the existing invocation Kernel and ToolCall port, rebuilds current managed discovery and executes the original tool through the same interface dispatch and host executable checks. Only matching embedded resources from a successful call are returned; no partial result is returned on failure. Reads repeat the original business read and can observe a changed file. Both encoding and decoding obey the existing 2 MiB tool-arguments transport budget, with strict versioned payload fields.


### Link-only and resource discovery contract

Optional object `resource_read_arguments` shallow-merges into public business arguments on reread, overriding matching keys without changing actor, workspace, instance, tool or des_id. Protocol rereads and Host import share this logic and preserve the same-installation serialized-source reentry guard. Resource block `skip_if_null` defaults to false: when enabled, only an explicitly null text/blob value skips the block; missing paths and malformed values still fail.

```json
{
  "structured_content_path": "/metadata",
  "resource_read": true,
  "resource_read_arguments": {"link_only": false},
  "resources": [{"name": "Report", "uri": "urn:report:current", "arguments": {"id": "current"}, "mime_type": "application/pdf"}],
  "mcp_content": [
    {"type": "resource_link", "uri_path": "/uri", "name_path": "/name"},
    {"type": "resource", "uri_path": "/uri", "blob_path": "/blob", "skip_if_null": true}
  ]
}
```

An initial `link_only:true` call can return `blob:null` and a link; reread forces false and returns the complete blob. Pure ResourceLink declarations project normally but cannot enable resource_read, preventing unreadable templates. Link-only refers to an initial null business body, not a mapping permanently lacking embedded content. Bodies never enter ephemeral storage; the existing `ResourceReference` wire is unchanged.

`resources/list` projects only explicit `resources` entries on currently visible, read-enabled InterfaceWrapper tools. Entries require name, uri and arguments, with optional mime_type; the URI is wrapped as a replay reference. An empty list is valid when no entries are declared. Listing never invokes a tool, scans disk or enumerates historical call results.

`resources/templates/list` emits a revision-bound template for each such visible tool: `flowbase-resource://template/<BASE64URL_IDENTITY>?arguments={arguments}&resource_uri={resource_uri}`. RFC 6570 simple variable expansion encodes a JSON object string for arguments and an original absolute resource URI for resource_uri. Identity contains only instance_id, tool_id and des_id; user/workspace come from the current request. Expanded reads reuse the existing locator codec, current authorization and canonical tool dispatch, returning exactly one matching embedded resource. Zero/multiple matches, host self-references, unknown/duplicate parameters and stale revisions fail.

Both directories sort by URI and paginate with a directory-kind/last-key cursor. Each request recomputes current visibility; pages including nextCursor use half the existing JSON envelope budget, leaving envelope headroom. There is no machine-capacity constant or cross-request catalog/binary cache; an oversized entry fails explicitly. Clients continue with nextCursor; visibility/catalog changes may affect subsequent pages.
