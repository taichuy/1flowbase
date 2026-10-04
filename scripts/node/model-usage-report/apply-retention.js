// Revision-fenced rollout for the three existing report blocks; no workflow writes.
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const { withApi, source, PAGE, BLOCK } = require("./apply");
const previousSources = {
  overview: "f29195b640668bbe7bb7eba33fb4e73a9be848bbcaded9dfcd965412452737e5",
  trend: "f0f88ca4855b408644d6b36dfa7bcb33f048e148236bdb98be3ed0bf67ce99a5",
  users: "f8d651952c4f0494a8b652d1281821e713c1aa18c3ee21dcfdc1b85334a15d5d",
};
const { retainedBlockPatch } = require("./retained-block");
const hash = (value) => crypto.createHash("sha256").update(value).digest("hex");

async function main() {
  const evidence = path.resolve(
    process.env.USAGE_EVIDENCE_DIR ||
      "tmp/test-governance/report-render/rollout",
    new Date().toISOString().replaceAll(":", "-"),
  );
  fs.mkdirSync(evidence, { recursive: true });
  const save = (name, value) =>
    fs.writeFileSync(
      path.join(evidence, name + ".json"),
      JSON.stringify(value, null, 2),
    );
  await withApi(async (api) => {
    const root = `/api/console/frontstage/pages/${PAGE}/blocks`;
    const overview = await api(root + "/" + BLOCK);
    const listing = await api(
      root + "?tab_id=" + overview.tab_id + "&limit=100",
    );
    const items = Array.isArray(listing) ? listing : listing.items;
    const plans = [];
    for (const file of ["overview", "trend", "users"]) {
      const block =
        file === "overview"
          ? overview
          : items.find(
              (item) => item.description === "model-usage-report:" + file,
            );
      if (!block) throw new Error("Missing existing report block: " + file);
      const code = await api(root + "/" + block.block_id + "/code");
      const next = source(file);
      if (
        hash(code.source_code) !== previousSources[file] &&
        hash(code.source_code) !== hash(next)
      )
        throw new Error(
          "Source changed since diagnosis; refusing to overwrite " + file,
        );
      plans.push({
        file,
        block,
        code,
        patch: retainedBlockPatch(block, file === "overview"),
        next,
      });
    }
    save("originals-and-plan", plans);
    for (const plan of plans) {
      await api(root + "/" + plan.block.block_id + "/code", "PUT", {
        source_code: plan.next,
        expected_source_revision: plan.code.source_sha256,
      });
      await api(root + "/" + plan.block.block_id, "PATCH", plan.patch);
    }
    save(
      "applied",
      plans.map(({ file, block, next }) => ({
        file,
        block_id: block.block_id,
        source_sha256: hash(next),
      })),
    );
    console.log(
      JSON.stringify({ page_id: PAGE, blocks: plans.length, evidence }),
    );
  });
}
if (require.main === module)
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
module.exports = { retainedBlockPatch };
