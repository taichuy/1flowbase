const fs = require('node:fs');
const path = require('node:path');
const { createHash } = require('node:crypto');

const REVIEW_FILE = 'scripts/node/react-doctor-gate/reviews.json';

function sourceDigest(content) {
  return createHash('sha256').update(content).digest('hex');
}

// Review data is grouped by source owner; missing/ambiguous groups fail closed.
function loadReviews(repoRoot, toolPackage) {
  try {
    const manifest = JSON.parse(fs.readFileSync(path.join(repoRoot, REVIEW_FILE), 'utf8'));
    if (manifest.toolPackage !== toolPackage) return [];
    if (manifest.schemaVersion === 1) return Array.isArray(manifest.reviews) ? manifest.reviews : [];
    if (manifest.schemaVersion !== 2 || !Array.isArray(manifest.reviewFiles)
        || new Set(manifest.reviewFiles).size !== manifest.reviewFiles.length) return [];
    return manifest.reviewFiles.flatMap((file) => {
      if (typeof file !== 'string' || path.posix.normalize(file) !== file
          || !file.startsWith('scripts/node/react-doctor-gate/reviews/') || !file.endsWith('.json')) {
        throw new Error('invalid review group path');
      }
      const group = JSON.parse(fs.readFileSync(path.join(repoRoot, file), 'utf8'));
      if (group.toolPackage !== toolPackage || !Array.isArray(group.reviews)) throw new Error('invalid review group');
      return group.reviews;
    });
  } catch {
    return [];
  }
}

// Keep every upstream diagnostic. A source-bound counterexample is review evidence,
// never an instruction to hide the rule or ignore future changes in that file.
function evaluateDiagnosticReviews({ repoRoot, doctorReport, toolPackage }) {
  const diagnostics = doctorReport?.diagnostics;
  if (!Array.isArray(diagnostics)
      || doctorReport?.summary?.totalDiagnosticCount !== diagnostics.length
      || doctorReport.error) {
    return { status: 'invalid-output', reviewed: 0, findings: [{ reason: 'Doctor must return a complete diagnostic inventory and matching total' }] };
  }
  if (doctorReport.summary.suppressedDiagnosticCount > 0) {
    return { status: 'suppressed-output', reviewed: 0, findings: [{ reason: 'Doctor reports suppressed diagnostics; raw inventory is incomplete for acceptance' }] };
  }
  const reviews = loadReviews(repoRoot, toolPackage);
  const byKey = new Map();
  for (const review of reviews) {
    const key = `${review.file}\0${review.rule}`;
    // Ambiguous review records must not accidentally authorize a diagnostic.
    byKey.set(key, byKey.has(key) ? null : review);
  }
  const findings = [];
  const digests = new Map();
  const digestFile = (file) => {
    if (!digests.has(file)) {
      try { digests.set(file, sourceDigest(fs.readFileSync(path.join(repoRoot, file)))); }
      catch { digests.set(file, null); }
    }
    return digests.get(file);
  };
  for (const diagnostic of diagnostics) {
    if (!diagnostic || typeof diagnostic.filePath !== 'string'
        || typeof diagnostic.plugin !== 'string' || typeof diagnostic.rule !== 'string') {
      findings.push({ reason: 'diagnostic is missing its source or rule identity' });
      continue;
    }
    const file = path.posix.normalize(`web/app/${diagnostic.filePath}`);
    const rule = `${diagnostic.plugin}/${diagnostic.rule}`;
    const review = byKey.get(`${file}\0${rule}`);
    let reason;
    if (!file.startsWith('web/app/') || diagnostic.filePath.startsWith('/') || diagnostic.filePath.includes('\\')) {
      reason = 'diagnostic path is outside the reviewed application source boundary';
    } else if (!review || typeof review.reason !== 'string' || !review.reason.trim() || !Array.isArray(review.evidence) || review.evidence.length === 0) {
      reason = 'diagnostic has no explicit source and contract review';
    } else {
      if (digestFile(file) !== review.sourceSha256) {
        reason = 'source changed after review; reassess the counterexample or repair';
      } else if (!review.evidence.every((entry) => {
        if (!entry || typeof entry.file !== 'string' || typeof entry.sha256 !== 'string') return false;
        const normalized = path.posix.normalize(entry.file);
        return normalized === entry.file && !entry.file.startsWith('../') && !path.isAbsolute(entry.file)
          && digestFile(entry.file) === entry.sha256;
      })) {
        reason = 'review evidence changed, disappeared, or references an invalid repository file';
      }
    }
    if (reason) findings.push({ file, rule, line: diagnostic.line, reason });
  }
  return {
    status: findings.length ? 'requires-review' : 'reviewed',
    reviewed: diagnostics.length - findings.length,
    findings,
  };
}

module.exports = { REVIEW_FILE, sourceDigest, evaluateDiagnosticReviews };
