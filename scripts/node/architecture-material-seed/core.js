const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const manifest = require("./manifest.json");
const {
  loadRootCredentials,
  openTemporaryOwnerSession,
} = require("../page-debug/auth.js");

const DEFAULT_API_BASE_URL = "http://127.0.0.1:7800";
const FIELD_SPECS = [
  ["source_key", "Source key", true, true],
  ["title", "Title", true, false],
  ["node_kind", "Node kind", true, false],
  ["ordinal", "Ordinal", true, false],
  ["archive_sha256", "Archive SHA-256", true, false],
  ["source_members", "Source members", true, false],
  ["source_start", "Source start", true, false],
  ["source_end", "Source end", true, false],
  ["source_sha256", "Source SHA-256", true, false],
  ["content", "Markdown content", true, false],
  ["frontstage_block_id", "Frontstage Block id", false, false],
];

function sha256(value) {
  return crypto.createHash("sha256").update(value).digest("hex");
}

function unzipMember(zipPath, memberName) {
  const result = spawnSync("unzip", ["-p", zipPath, memberName], {
    encoding: null,
    maxBuffer: 4 * 1024 * 1024,
  });
  if (result.error) {
    throw new Error(`无法执行 unzip：${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(
      `无法读取 archive member ${memberName}：${String(result.stderr).slice(0, 500)}`,
    );
  }
  return result.stdout;
}

function listArchiveMembers(zipPath) {
  const result = spawnSync("unzip", ["-Z1", zipPath], {
    encoding: "utf8",
    maxBuffer: 1024 * 1024,
  });
  if (result.error) throw new Error(`无法执行 unzip：${result.error.message}`);
  if (result.status !== 0) {
    throw new Error(
      `无法读取 archive inventory：${result.stderr.slice(0, 500)}`,
    );
  }
  return result.stdout.split(/\r?\n/u).filter(Boolean);
}

function memberLine(member, absoluteOffset) {
  const localOffset = Math.max(
    0,
    Math.min(absoluteOffset - member.start, member.text.length),
  );
  let line = 1;
  for (let index = 0; index < localOffset; index += 1) {
    if (member.text.charCodeAt(index) === 10) line += 1;
  }
  return `${member.name}:${line}`;
}

function memberAt(members, absoluteOffset) {
  return members.find(
    (member, index) =>
      absoluteOffset >= member.start &&
      (absoluteOffset < member.end ||
        (index === members.length - 1 && absoluteOffset === member.end)),
  );
}

function sourceSpan(members, start, end) {
  const safeEnd = Math.max(start, end - 1);
  const first = memberAt(members, start);
  const last = memberAt(members, safeEnd);
  if (!first || !last) throw new Error(`无法定位 source span ${start}:${end}`);
  const names = members
    .filter((member) => member.end > start && member.start < end)
    .map((member) => member.name);
  return {
    sourceMembers: names.join(","),
    sourceStart: memberLine(first, start),
    sourceEnd: memberLine(last, safeEnd),
  };
}

function parseMaterialTree(source, members, lockedManifest = manifest) {
  const headings = [];
  const lines = /^.*(?:\n|$)/gmu;
  let match;
  let activeChapter = 0;
  let nextChapter = 1;

  while ((match = lines.exec(source)) !== null) {
    if (!match[0]) break;
    const line = match[0].replace(/\r?\n$/u, "");
    const chapter = line.match(
      /^#{1,2}[ \t]*第[ \t]*(\d+)[ \t]*章[ \t]*(.*?)[ \t]*$/u,
    );
    if (chapter && Number(chapter[1]) === nextChapter) {
      activeChapter = nextChapter;
      nextChapter += 1;
      headings.push({
        kind: "chapter",
        chapter: activeChapter,
        section: null,
        offset: match.index,
        title: `第${activeChapter}章 ${chapter[2].trim()}`,
        sourceKey: `chapter-${String(activeChapter).padStart(2, "0")}`,
        parentKey: lockedManifest.targets.root_source_key,
      });
      continue;
    }

    const section = line.match(
      /^##[ \t]*(\d+)\.(\d+)(?![\d.])[ \t]*(.*?)[ \t]*$/u,
    );
    if (section && Number(section[1]) === activeChapter) {
      const sectionNumber = Number(section[2]);
      headings.push({
        kind: "section",
        chapter: activeChapter,
        section: sectionNumber,
        offset: match.index,
        title: `${activeChapter}.${sectionNumber} ${section[3].trim()}`,
        sourceKey: `section-${String(activeChapter).padStart(2, "0")}-${String(sectionNumber).padStart(2, "0")}`,
        parentKey: `chapter-${String(activeChapter).padStart(2, "0")}`,
      });
    }
  }

  if (headings[0]?.kind !== "chapter" || headings[0]?.chapter !== 1) {
    throw new Error("授权源中未找到正文第1章起点");
  }

  const boundaries = [
    {
      kind: "root",
      chapter: null,
      section: null,
      offset: 0,
      title: "教材编辑",
      sourceKey: lockedManifest.targets.root_source_key,
      parentKey: null,
    },
    ...headings,
  ];
  const nodes = boundaries.map((boundary, index) => {
    const end = boundaries[index + 1]?.offset ?? source.length;
    const content = source.slice(boundary.offset, end);
    const span = sourceSpan(members, boundary.offset, end);
    return {
      ...boundary,
      ordinal: index,
      content,
      sourceSha256: sha256(Buffer.from(content, "utf8")),
      ...span,
    };
  });

  const sectionsPerChapter = Array.from(
    { length: nextChapter - 1 },
    (_, index) =>
      nodes.filter(
        (node) => node.kind === "section" && node.chapter === index + 1,
      ).length,
  );
  assertDerivedShape(nodes, sectionsPerChapter, lockedManifest);
  return { nodes, sectionsPerChapter };
}

function assertDerivedShape(
  nodes,
  sectionsPerChapter,
  lockedManifest = manifest,
) {
  const counts = {
    root: nodes.filter((node) => node.kind === "root").length,
    chapter: nodes.filter((node) => node.kind === "chapter").length,
    section: nodes.filter((node) => node.kind === "section").length,
  };
  const expected = lockedManifest.derived;
  if (
    counts.root !== expected.root_count ||
    counts.chapter !== expected.chapter_count ||
    counts.section !== expected.section_count ||
    nodes.length !== expected.node_count ||
    JSON.stringify(sectionsPerChapter) !==
      JSON.stringify(expected.sections_per_chapter)
  ) {
    throw new Error(
      `源结构与 manifest 不一致：${JSON.stringify({ counts, sectionsPerChapter })}`,
    );
  }
  const keys = new Set(nodes.map((node) => node.sourceKey));
  if (keys.size !== nodes.length)
    throw new Error("解析结果包含重复 source_key");
}

function loadAndParseArchive(zipPath, lockedManifest = manifest) {
  const archive = fs.readFileSync(zipPath);
  const archiveSha256 = sha256(archive);
  if (archiveSha256 !== lockedManifest.source.archive_sha256) {
    throw new Error(`archive SHA-256 不匹配：${archiveSha256}`);
  }

  const expectedInventory = lockedManifest.source.members
    .map((member) => member.name)
    .sort();
  const actualInventory = listArchiveMembers(zipPath).sort();
  if (JSON.stringify(actualInventory) !== JSON.stringify(expectedInventory)) {
    throw new Error(
      `archive member inventory 不匹配：${JSON.stringify(actualInventory)}`,
    );
  }

  let offset = 0;
  const members = lockedManifest.source.members.map((expected) => {
    const buffer = unzipMember(zipPath, expected.name);
    const actualSha = sha256(buffer);
    if (buffer.length !== expected.bytes || actualSha !== expected.sha256) {
      throw new Error(
        `archive member 不匹配：${expected.name} bytes=${buffer.length} sha256=${actualSha}`,
      );
    }
    const text = buffer.toString("utf8");
    const member = {
      name: expected.name,
      buffer,
      text,
      start: offset,
      end: offset + text.length,
    };
    offset = member.end;
    return member;
  });
  const combinedBuffer = Buffer.concat(members.map((member) => member.buffer));
  if (combinedBuffer.length !== lockedManifest.source.uncompressed_bytes) {
    throw new Error(`解压总字节数不匹配：${combinedBuffer.length}`);
  }
  const concatenatedSha256 = sha256(combinedBuffer);
  if (concatenatedSha256 !== lockedManifest.source.concatenated_sha256) {
    throw new Error(`拼接内容 SHA-256 不匹配：${concatenatedSha256}`);
  }
  const source = members.map((member) => member.text).join("");
  const parsed = parseMaterialTree(source, members, lockedManifest);
  const parsedBytes = parsed.nodes.reduce(
    (total, node) => total + Buffer.byteLength(node.content),
    0,
  );
  if (parsedBytes !== combinedBuffer.length) {
    throw new Error(`派生节点未完整覆盖授权源：${parsedBytes}`);
  }
  return { archiveSha256, concatenatedSha256, members, ...parsed };
}

function resolveAcceptanceZipPath({ repoRoot, sourceEnv = process.env }) {
  const candidates = [
    sourceEnv.ARCHITECTURE_MATERIAL_ZIP,
    path.join(repoRoot, manifest.source.logical_path),
    path.resolve(repoRoot, "..", "1flowbase", manifest.source.logical_path),
    path.resolve(
      repoRoot,
      "..",
      "..",
      "1flowbase",
      manifest.source.logical_path,
    ),
  ].filter(Boolean);
  return (
    candidates.find((candidate) => fs.existsSync(candidate)) || candidates[0]
  );
}

function normalizeBaseUrl(value) {
  return value.replace(/\/$/u, "");
}

async function apiRequest(
  client,
  apiPath,
  { method = "GET", body, expected = [200] } = {},
) {
  const headers = { cookie: client.cookie };
  if (body !== undefined) headers["content-type"] = "application/json";
  if (method !== "GET" && method !== "HEAD")
    headers["x-csrf-token"] = client.csrfToken;
  const response = await client.fetchImpl(`${client.apiBaseUrl}${apiPath}`, {
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await response.text();
  let payload = null;
  try {
    payload = text ? JSON.parse(text) : null;
  } catch {
    throw new Error(
      `${method} ${apiPath} 返回非 JSON：${response.status} ${text.slice(0, 300)}`,
    );
  }
  if (!expected.includes(response.status)) {
    throw new Error(
      `${method} ${apiPath} 失败：${response.status} ${text.slice(0, 1000)}`,
    );
  }
  return payload?.data ?? payload;
}

async function ensureModel(client) {
  const models = await apiRequest(
    client,
    "/api/console/settings/data-models/model-definitions",
  );
  let model = models.find(
    (candidate) => candidate.code === manifest.targets.model_code,
  );
  if (!model) {
    model = await apiRequest(
      client,
      "/api/console/settings/data-models/model-definitions",
      {
        method: "POST",
        expected: [201],
        body: {
          scope_kind: "workspace",
          template_provider: "core",
          template_code: "ordered_tree",
          template_version: "v1",
          code: manifest.targets.model_code,
          title: "架构教材节点",
          description: `Authorized local acceptance seed from ${manifest.source.logical_path}`,
        },
      },
    );
  }
  if (
    model.template_provider !== "core" ||
    model.template_code !== "ordered_tree" ||
    model.template_version !== "v1"
  ) {
    throw new Error(
      `${manifest.targets.model_code} 已存在但不是 core/ordered_tree/v1`,
    );
  }

  const fieldsByCode = new Map(
    model.fields.map((field) => [field.code, field]),
  );
  for (const [code, title, isRequired, isUnique] of FIELD_SPECS) {
    const existing = fieldsByCode.get(code);
    if (existing) {
      if (
        existing.field_kind !== "text" ||
        existing.is_required !== isRequired ||
        existing.is_unique !== isUnique
      ) {
        throw new Error(`既有字段 contract 不匹配：${code}`);
      }
      continue;
    }
    const created = await apiRequest(
      client,
      `/api/console/settings/data-models/model-definitions/${encodeURIComponent(model.id)}/fields`,
      {
        method: "POST",
        expected: [201],
        body: {
          code,
          title,
          field_kind: "text",
          is_required: isRequired,
          is_unique: isUnique,
          display_options: {},
        },
      },
    );
    fieldsByCode.set(code, created);
  }
  return model;
}

function recordPayload(node) {
  return {
    source_key: node.sourceKey,
    title: node.title,
    node_kind: node.kind === "section" ? "content" : node.kind,
    ordinal: String(node.ordinal),
    archive_sha256: manifest.source.archive_sha256,
    source_members: node.sourceMembers,
    source_start: node.sourceStart,
    source_end: node.sourceEnd,
    source_sha256: node.sourceSha256,
    content: node.content,
  };
}

function validateRecord(record, node) {
  for (const [key, expected] of Object.entries(recordPayload(node))) {
    if (record[key] !== expected) {
      throw new Error(
        `既有教材 record 与授权源不一致：${node.sourceKey}.${key}`,
      );
    }
  }
}

async function ensureRecords(client, nodes) {
  const code = encodeURIComponent(manifest.targets.model_code);
  const roots = await apiRequest(
    client,
    `/api/runtime/models/${code}/tree/roots?limit=1000`,
  );
  const root = roots.find(
    (record) => record.source_key === manifest.targets.root_source_key,
  );
  const recordsByKey = new Map();
  if (root) {
    recordsByKey.set(root.source_key, root);
    const descendants = await apiRequest(
      client,
      `/api/runtime/models/${code}/tree/descendants/${encodeURIComponent(root.id)}?max_depth=2&limit=1000`,
    );
    for (const projection of descendants) {
      recordsByKey.set(projection.record.source_key, projection.record);
    }
    for (const key of recordsByKey.keys()) {
      if (!nodes.some((node) => node.sourceKey === key)) {
        throw new Error(`既有教材 tree 含 manifest 之外的节点：${key}`);
      }
    }
  }

  for (const node of nodes) {
    let record = recordsByKey.get(node.sourceKey);
    const parent = node.parentKey ? recordsByKey.get(node.parentKey) : null;
    if (record) {
      validateRecord(record, node);
      if (record.parent_id !== (parent?.id ?? null)) {
        throw new Error(`既有教材 record 层级不匹配：${node.sourceKey}`);
      }
      continue;
    }
    if (node.parentKey && !parent)
      throw new Error(`缺少父 record：${node.parentKey}`);
    record = await apiRequest(client, `/api/runtime/models/${code}/create`, {
      method: "POST",
      expected: [201],
      body: {
        ...recordPayload(node),
        ...(parent ? { parent_id: parent.id } : {}),
      },
    });
    recordsByKey.set(node.sourceKey, record);
  }
  return recordsByKey;
}

function flattenPages(nodes) {
  return nodes.flatMap((node) => [node, ...flattenPages(node.children || [])]);
}

async function ensurePage(client, workspaceId) {
  const encodedWorkspace = encodeURIComponent(workspaceId);
  const pages = await apiRequest(
    client,
    `/api/console/frontstage/${encodedWorkspace}/pages`,
  );
  let page = flattenPages(pages).find(
    (candidate) => candidate.slug === manifest.targets.page_slug,
  );
  let defaultTab;
  if (!page) {
    const created = await apiRequest(
      client,
      `/api/console/frontstage/${encodedWorkspace}/pages`,
      {
        method: "POST",
        expected: [201],
        body: {
          title: "资料库",
          placement: "topbar",
          slug: manifest.targets.page_slug,
        },
      },
    );
    page = created;
    defaultTab = created.default_tab;
  } else {
    if (page.kind !== "page")
      throw new Error(`${manifest.targets.page_slug} 不是 Frontstage page`);
    const tabs = await apiRequest(
      client,
      `/api/console/frontstage/${encodedWorkspace}/pages/${encodeURIComponent(page.id)}/tabs`,
    );
    defaultTab = tabs.find((tab) => tab.is_default);
  }
  if (!defaultTab) throw new Error("资料库 page 缺少 default tab");
  return { page, defaultTab };
}

function blockDescriptor(node, record) {
  return {
    acceptanceSeed: "architecture-material-editor",
    archiveSha256: manifest.source.archive_sha256,
    dataModelCode: manifest.targets.model_code,
    recordId: record.id,
    sourceKey: node.sourceKey,
    nodeKind: node.kind === "section" ? "content" : node.kind,
  };
}

function blockSource(recordId) {
  const endpoint = `/api/runtime/models/${manifest.targets.model_code}/get/${recordId}`;
  return `import { useEffect, useState } from 'react';

type MaterialRecord = { title?: string; content?: string };

export default function ArchitectureMaterial({ ctx }: NativeReactBlockProps) {
  const [record, setRecord] = useState<MaterialRecord | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    ctx.api.get<MaterialRecord>(${JSON.stringify(endpoint)})
      .then((value) => { if (active) setRecord(value); })
      .catch((reason) => { if (active) setError(String(reason)); });
    return () => { active = false; };
  }, [ctx]);

  if (error) return <div role="alert">{error}</div>;
  if (!record) return <div>Loading…</div>;
  return (
    <article style={{ maxWidth: 960, margin: '0 auto', padding: 24 }}>
      <h1>{record.title ?? '教材内容'}</h1>
      <pre style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere', fontFamily: 'inherit' }}>
        {record.content ?? ''}
      </pre>
    </article>
  );
}
`;
}

function validateBlock(block, node, record, expectedParentId) {
  const descriptor = block.runtime_descriptor || {};
  if (
    block.parent_block_id !== expectedParentId ||
    descriptor.acceptanceSeed !== "architecture-material-editor" ||
    descriptor.archiveSha256 !== manifest.source.archive_sha256 ||
    descriptor.recordId !== record.id ||
    descriptor.sourceKey !== node.sourceKey
  ) {
    throw new Error(`既有 Frontstage Block link 不匹配：${node.sourceKey}`);
  }
}

async function findExistingBlock(
  client,
  basePath,
  node,
  record,
  expectedParentId,
) {
  const results = await apiRequest(
    client,
    `${basePath}/search?query=${encodeURIComponent(node.title)}&limit=100`,
  );
  for (const result of results) {
    if (
      result.node.title !== node.title ||
      result.node.parent_block_id !== expectedParentId
    )
      continue;
    const block = await apiRequest(
      client,
      `${basePath}/${encodeURIComponent(result.node.block_id)}`,
    );
    if (block.runtime_descriptor?.recordId === record.id) return block;
  }
  return null;
}

async function ensureBlocks(
  client,
  workspaceId,
  page,
  defaultTab,
  nodes,
  recordsByKey,
) {
  const basePath = `/api/console/frontstage/${encodeURIComponent(workspaceId)}/pages/${encodeURIComponent(page.id)}/blocks`;
  const blocksByKey = new Map();
  for (const node of nodes) {
    const record = recordsByKey.get(node.sourceKey);
    const parentBlock = node.parentKey ? blocksByKey.get(node.parentKey) : null;
    const expectedParentId = parentBlock?.block_id ?? null;
    let block = null;
    if (record.frontstage_block_id) {
      block = await apiRequest(
        client,
        `${basePath}/${encodeURIComponent(record.frontstage_block_id)}`,
        { expected: [200, 404] },
      );
      if (!block?.block_id) block = null;
    } else {
      block = await findExistingBlock(
        client,
        basePath,
        node,
        record,
        expectedParentId,
      );
    }
    if (!block) {
      block = await apiRequest(client, basePath, {
        method: "POST",
        expected: [201],
        body: {
          tab_id: defaultTab.id,
          title: node.title,
          presentation: node.kind === "root" ? "page" : "inline",
          parent_block_id: expectedParentId,
          before_block_id: null,
          after_block_id: null,
          code: blockSource(record.id),
          runtime_descriptor: blockDescriptor(node, record),
        },
      });
    }
    validateBlock(block, node, record, expectedParentId);
    blocksByKey.set(node.sourceKey, block);
    if (record.frontstage_block_id !== block.block_id) {
      const updated = await apiRequest(
        client,
        `/api/runtime/models/${encodeURIComponent(manifest.targets.model_code)}/update/${encodeURIComponent(record.id)}`,
        {
          method: "PATCH",
          body: { frontstage_block_id: block.block_id },
        },
      );
      recordsByKey.set(node.sourceKey, updated);
    }
  }
  const rootBlock = blocksByKey.get(manifest.targets.root_source_key);
  const open = await apiRequest(
    client,
    `${basePath}/${encodeURIComponent(rootBlock.block_id)}/open`,
  );
  return { blocksByKey, rootBlock, canonicalUrl: open.canonical_url };
}

function dryRunResult(parsed, zipPath) {
  return {
    mode: "dry-run",
    zip_path: path.resolve(zipPath),
    archive_sha256: parsed.archiveSha256,
    root_count: parsed.nodes.filter((node) => node.kind === "root").length,
    chapter_count: parsed.nodes.filter((node) => node.kind === "chapter")
      .length,
    section_count: parsed.nodes.filter((node) => node.kind === "section")
      .length,
    node_count: parsed.nodes.length,
    sections_per_chapter: parsed.sectionsPerChapter,
    discrepancy: manifest.derived.discrepancy,
  };
}

async function seedArchitectureMaterials(options, deps = {}) {
  const parsed = loadAndParseArchive(options.zipPath, manifest);
  if (options.dryRun) return dryRunResult(parsed, options.zipPath);

  const repoRoot = deps.repoRoot || path.resolve(__dirname, "..", "..", "..");
  const fetchImpl = deps.fetchImpl || globalThis.fetch;
  const credentials = (deps.loadRootCredentials || loadRootCredentials)({
    repoRoot,
    accountOverride: options.account,
    passwordOverride: options.password,
  });
  const owner = await (
    deps.openTemporaryOwnerSession || openTemporaryOwnerSession
  )({
    apiBaseUrl: options.apiBaseUrl,
    account: credentials.account,
    password: credentials.password,
    fetchImpl,
  });
  try {
    const client = {
      apiBaseUrl: normalizeBaseUrl(options.apiBaseUrl),
      cookie: owner.cookie,
      csrfToken: owner.csrfToken,
      fetchImpl,
    };
    const session = await apiRequest(client, "/api/console/session");
    const workspaceId =
      options.workspaceId || session.actor?.current_workspace_id;
    if (!workspaceId)
      throw new Error("console session 缺少 current_workspace_id");
    const model = await ensureModel(client);
    const recordsByKey = await ensureRecords(client, parsed.nodes);
    const { page, defaultTab } = await ensurePage(client, workspaceId);
    const blocks = await ensureBlocks(
      client,
      workspaceId,
      page,
      defaultTab,
      parsed.nodes,
      recordsByKey,
    );
    return {
      mode: "seed",
      workspace_id: workspaceId,
      model_id: model.id,
      model_code: manifest.targets.model_code,
      page_id: page.id,
      page_slug: manifest.targets.page_slug,
      root_record_id: recordsByKey.get(manifest.targets.root_source_key).id,
      root_block_id: blocks.rootBlock.block_id,
      node_count: parsed.nodes.length,
      canonical_url: blocks.canonicalUrl,
    };
  } finally {
    await owner.dispose();
  }
}

module.exports = {
  DEFAULT_API_BASE_URL,
  apiRequest,
  blockSource,
  dryRunResult,
  loadAndParseArchive,
  parseMaterialTree,
  resolveAcceptanceZipPath,
  seedArchitectureMaterials,
};
