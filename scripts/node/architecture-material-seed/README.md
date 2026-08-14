# Architecture material acceptance seed

This bounded local importer turns the authorized `xitonjiagoushi.zip` into one
`architecture_teaching_material_nodes` Ordered Tree and a linked Frontstage Block tree.
The zip remains canonical input; expanded Markdown is never written or committed.

The locked manifest records the archive/member hashes and the source-derived result:
1 root, 20 chapters, and 119 sections. This intentionally reports and supersedes the
earlier 17/104 expectation, which the authorized second-edition source does not support.

Validate without API writes:

```bash
node scripts/node/architecture-material-seed/cli.js \
  --dry-run \
  --zip /home/taichuy/git/1flowbase/docs/ziliao/xitonjiagoushi.zip
```

Seed or resume against the local runtime:

```bash
node scripts/node/architecture-material-seed/cli.js \
  --zip /home/taichuy/git/1flowbase/docs/ziliao/xitonjiagoushi.zip
```

The importer reuses the repository's temporary console session owner, creates or verifies
the Ordered Tree model and fields, creates or resumes source-keyed records, creates or
repairs their linked Blocks under the `architecture-materials` page, and prints the root
Block's canonical deep link. Existing source-linked data must match the manifest exactly;
on mismatch the importer stops instead of rewriting user data.
