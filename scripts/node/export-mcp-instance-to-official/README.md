# Export an MCP instance to the official source tree

This CLI authenticates with a temporary owner session, exports exactly one MCP instance, validates the returned ZIP with the official repository source validator, and atomically replaces the target source directory.

```bash
node scripts/node/export-mcp-instance-to-official.js \
  --instance-id 1flowbase \
  --target /home/taichuy/git/1flowbase-official-plugins/mcp/@taichuy/1flowbase_zh_hans \
  --api-base-url http://127.0.0.1:7800
```

`--api-base-url` defaults to `http://127.0.0.1:7800`. Use `-h` or `--help` to print the command usage without contacting the API.

The target `manifest.json` owns `organization`, `bundle_id`, `locale`, and the current semantic version. The CLI increments the patch version. The API owns both system-version fields; the CLI rejects an export unless `minimum_host_version` equals `exported_from_system_version`.

The command never commits or pushes either repository. On validation or replacement failure, the previous target directory remains intact.
