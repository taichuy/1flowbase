'use strict';

const fs = require('node:fs');
const path = require('node:path');

const MAX_INPUT_BYTES = 1024 * 1024;
const MAX_SAMPLES_PER_VARIANT = 200;
const VARIANTS = ['direct', 'gateway'];
const METRICS = ['total', 'ingress', 'mapping', 'flow', 'queue', 'connect', 'upstream', 'flush'];

function percentile(values, fraction) {
  if (values.length === 0) return null;
  const sorted = [...values].sort((left, right) => left - right);
  const rank = Math.ceil(fraction * sorted.length) - 1;
  return sorted[Math.max(0, rank)];
}

function validateSample(sample) {
  const keys = Object.keys(sample).sort();
  if (keys.join(',') !== 'metrics_ms,sample_index,variant') {
    throw new Error('sample accepts only variant, sample_index, and metrics_ms');
  }
  if (!VARIANTS.includes(sample.variant)) throw new Error('variant must be direct or gateway');
  if (!Number.isInteger(sample.sample_index) || sample.sample_index < 0) {
    throw new Error('sample_index must be a non-negative integer');
  }
  if (!sample.metrics_ms || typeof sample.metrics_ms !== 'object' || Array.isArray(sample.metrics_ms)) {
    throw new Error('metrics_ms must be an object');
  }
  for (const [name, value] of Object.entries(sample.metrics_ms)) {
    if (!METRICS.includes(name)) throw new Error(`unsupported timing metric: ${name}`);
    if (value !== null && (!Number.isFinite(value) || value < 0 || value > 86_400_000)) {
      throw new Error(`${name} must be null or a bounded non-negative millisecond value`);
    }
  }
}

function summarize(samples) {
  const grouped = Object.fromEntries(VARIANTS.map((variant) => [variant, []]));
  for (const sample of samples) {
    validateSample(sample);
    grouped[sample.variant].push(sample);
    if (grouped[sample.variant].length > MAX_SAMPLES_PER_VARIANT) {
      throw new Error(`${sample.variant} exceeds ${MAX_SAMPLES_PER_VARIANT} samples`);
    }
  }
  const variants = {};
  for (const variant of VARIANTS) {
    const metrics = {};
    for (const metric of METRICS) {
      const values = grouped[variant]
        .map((sample) => sample.metrics_ms[metric])
        .filter((value) => Number.isFinite(value));
      metrics[metric] = {
        sample_count: values.length,
        p50_ms: percentile(values, 0.50),
        p95_ms: percentile(values, 0.95),
      };
    }
    variants[variant] = { sample_count: grouped[variant].length, metrics };
  }
  return {
    schema_version: 1,
    benchmark_kind: 'provider_timing_direct_gateway_ab',
    max_samples_per_variant: MAX_SAMPLES_PER_VARIANT,
    variants,
  };
}

function dryRunSamples() {
  const samples = [];
  for (const [variant, totals] of Object.entries({
    direct: [90, 100, 110, 120, 130],
    gateway: [105, 115, 125, 135, 145],
  })) {
    totals.forEach((total, sampleIndex) => samples.push({
      variant,
      sample_index: sampleIndex,
      metrics_ms: {
        total,
        ingress: null,
        mapping: variant === 'gateway' ? 2 : null,
        flow: variant === 'gateway' ? 4 : null,
        queue: variant === 'gateway' ? 1 : null,
        connect: sampleIndex === 0 ? 12 : null,
        upstream: total - 15,
        flush: variant === 'gateway' ? 1 : null,
      },
    }));
  }
  return samples;
}

function readSamples(inputPath) {
  const bytes = fs.readFileSync(inputPath);
  if (bytes.length > MAX_INPUT_BYTES) throw new Error(`input exceeds ${MAX_INPUT_BYTES} bytes`);
  const value = JSON.parse(bytes.toString('utf8'));
  if (!Array.isArray(value)) throw new Error('input must be a JSON array');
  return value;
}

function main(argv) {
  if (argv.length === 1 && argv[0] === '--schema') {
    process.stdout.write(`${fs.readFileSync(path.join(__dirname, 'result.schema.json'), 'utf8').trim()}\n`);
    return;
  }
  let samples;
  if (argv.length === 1 && argv[0] === '--dry-run') samples = dryRunSamples();
  else if (argv.length === 2 && argv[0] === '--input') samples = readSamples(argv[1]);
  else throw new Error('usage: benchmark.js --dry-run | --schema | --input <sanitized-samples.json>');
  process.stdout.write(`${JSON.stringify(summarize(samples), null, 2)}\n`);
}

if (require.main === module) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  }
}

module.exports = { MAX_SAMPLES_PER_VARIANT, dryRunSamples, summarize, validateSample };
