const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "flowbase-nginx-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const config = path.join(root, "config");
  const template = path.join(root, "template.conf");
  const active = path.join(root, "active.conf");
  const checked = path.join(root, "checked");
  fs.writeFileSync(template, "image-v1");
  fs.writeFileSync(
    path.join(root, "nginx"),
    '#!/bin/sh\n[ "$1" = "-t" ] || exit 9\nprintf checked > "$CHECKED"\nexit "${CHECK_EXIT:-0}"\n',
    { mode: 0o755 },
  );
  const run = (env = {}) =>
    spawnSync(
      "sh",
      [
        path.resolve(__dirname, "../configure-nginx.sh"),
        config,
        template,
        active,
      ],
      {
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${root}:${process.env.PATH}`,
          CHECKED: checked,
          ...env,
        },
      },
    );
  return { root, config, template, active, checked, run };
}

function success(result) {
  assert.equal(result.status, 0, result.stderr);
}

test("empty directory exports a regular template, activates defaults and validates nginx", (t) => {
  const f = fixture(t);
  success(f.run());
  assert.equal(
    fs.readFileSync(path.join(f.config, "nginx.default.conf"), "utf8"),
    "image-v1",
  );
  assert.equal(fs.readFileSync(f.active, "utf8"), "image-v1");
  assert.equal(fs.existsSync(path.join(f.config, "nginx.conf")), false);
  assert.equal(fs.readFileSync(f.checked, "utf8"), "checked");
});

test("recreation and image upgrades refresh defaults without pinning the previous template", (t) => {
  const f = fixture(t);
  success(f.run());
  fs.unlinkSync(f.active);
  fs.writeFileSync(f.template, "image-v2");
  success(f.run());
  assert.equal(fs.readFileSync(f.active, "utf8"), "image-v2");
  assert.equal(
    fs.readFileSync(path.join(f.config, "nginx.default.conf"), "utf8"),
    "image-v2",
  );
});

test("custom configuration survives upgrade; removing it restores current image defaults", (t) => {
  const f = fixture(t);
  success(f.run());
  const custom = path.join(f.config, "nginx.conf");
  fs.writeFileSync(custom, "operator configuration");
  fs.writeFileSync(f.template, "image-v2");
  success(f.run());
  assert.equal(fs.readFileSync(custom, "utf8"), "operator configuration");
  assert.equal(fs.readFileSync(f.active, "utf8"), "operator configuration");
  assert.equal(
    fs.readFileSync(path.join(f.config, "nginx.default.conf"), "utf8"),
    "image-v2",
  );
  fs.unlinkSync(custom);
  success(f.run());
  assert.equal(fs.readFileSync(f.active, "utf8"), "image-v2");
});

test("a directory mistaken for an override fails with an actionable message", (t) => {
  const f = fixture(t);
  fs.mkdirSync(path.join(f.config, "nginx.conf"), { recursive: true });
  const result = f.run();
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /must be a readable file/);
  assert.equal(fs.existsSync(f.active), false);
});

test("nginx validation failure stops startup and preserves the custom file", (t) => {
  const f = fixture(t);
  fs.mkdirSync(f.config);
  const custom = path.join(f.config, "nginx.conf");
  fs.writeFileSync(custom, "invalid syntax");
  assert.equal(f.run({ CHECK_EXIT: "1" }).status, 1);
  assert.equal(fs.readFileSync(custom, "utf8"), "invalid syntax");
});

for (const file of [
  "docker-compose.yaml",
  "docker-compose.dev.yaml",
  "docker-compose.external-db.yaml",
  "deployment/compose.yaml",
  "deployment/compose.aliyun.yaml",
]) {
  test(`${file} mounts a configuration directory instead of a missing file`, () => {
    const compose = fs.readFileSync(
      path.resolve(__dirname, "../../", file),
      "utf8",
    );
    assert.match(compose, /\.\/web\/nginx:\/config\/nginx/);
    assert.doesNotMatch(
      compose,
      /\.\/web\/nginx\.conf:\/etc\/nginx\/conf.d\/default.conf/,
    );
  });
}
