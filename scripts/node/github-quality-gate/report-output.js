const fs = require('node:fs');

const MAX_GITHUB_ISSUE_BODY_BYTES = 60 * 1024;
// GitHub documents a 1 MiB file limit per step summary, independent of UTF-16 length.
const GITHUB_STEP_SUMMARY_MAX_BYTES = 1024 * 1024;

function boundUtf8(markdown, maxBytes, suffix) {
  const body = String(markdown || '');
  if (Buffer.byteLength(body, 'utf8') <= maxBytes) return body;

  const prefixBudget = maxBytes - Buffer.byteLength(suffix, 'utf8');
  let low = 0;
  let high = body.length;
  while (low < high) {
    const middle = Math.ceil((low + high) / 2);
    if (Buffer.byteLength(body.slice(0, middle), 'utf8') <= prefixBudget) low = middle;
    else high = middle - 1;
  }
  if (
    low > 0
    && low < body.length
    && /[\uD800-\uDBFF]/u.test(body[low - 1])
    && /[\uDC00-\uDFFF]/u.test(body[low])
  ) low -= 1;
  return `${body.slice(0, low).trimEnd()}${suffix}`;
}

function boundIssueBody(markdown, maxBytes = MAX_GITHUB_ISSUE_BODY_BYTES) {
  const suffix = [
    '',
    '---',
    'Issue report truncated to GitHub’s body limit. Complete component logs and failure details remain in the workflow artifact.',
    '',
  ].join('\n');
  return boundUtf8(markdown, maxBytes, suffix);
}

function appendStepSummary(markdown, summaryPath = process.env.GITHUB_STEP_SUMMARY) {
  if (!summaryPath) return;
  const existingBytes = fs.existsSync(summaryPath) ? fs.statSync(summaryPath).size : 0;
  const suffix = '\n\nStep summary truncated to GitHub’s 1 MiB limit. The complete report and logs remain in the workflow artifact.\n';
  const available = GITHUB_STEP_SUMMARY_MAX_BYTES - existingBytes - 2;
  if (available < Buffer.byteLength(suffix, 'utf8')) return;
  fs.appendFileSync(summaryPath, `\n${boundUtf8(markdown, available, suffix)}\n`, 'utf8');
}

module.exports = { boundIssueBody, appendStepSummary, GITHUB_STEP_SUMMARY_MAX_BYTES };
