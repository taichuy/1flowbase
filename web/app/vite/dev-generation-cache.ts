import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

import lockfile from 'proper-lockfile';

const GENERATION_PATTERN = /^[a-f0-9]{64}$/u;
const OWNER_PATTERN = /^([1-9]\d*)-[a-f0-9-]{36}$/u;
const GENERATIONS_RETAINED = 2;

function generationsDirectory(root: string) {
  return path.join(root, 'node_modules', '.vite-generations');
}

async function withGenerationLock<T>(
  root: string,
  operation: (directory: string) => Promise<T> | T
): Promise<T> {
  const directory = generationsDirectory(root);
  fs.mkdirSync(directory, { recursive: true });
  // Registration and collection share one cross-process critical section.
  // Use the library's heartbeat/stale-lock recovery rather than a PID mutex.
  const release = await lockfile.lock(directory, {
    retries: { retries: 10, minTimeout: 50, maxTimeout: 500 }
  });
  try {
    return await operation(directory);
  } finally {
    await release();
  }
}

export async function retainDevGenerationCache(
  root: string,
  generation: string
) {
  if (!GENERATION_PATTERN.test(generation))
    throw new Error('Invalid dev generation');
  const owner = `${process.pid}-${crypto.randomUUID()}`;
  await withGenerationLock(root, (directory) => {
    const ownersDirectory = path.join(directory, '.owners', generation);
    fs.mkdirSync(ownersDirectory, { recursive: true });
    // The PID is in the filename: a killed writer cannot leave partial JSON.
    fs.writeFileSync(path.join(ownersDirectory, owner), '', { flag: 'wx' });
    fs.mkdirSync(path.join(directory, generation), { recursive: true });
  });
  return () =>
    withGenerationLock(root, (directory) => {
      fs.rmSync(path.join(directory, '.owners', generation, owner), {
        force: true
      });
    });
}

function hasLiveOwners(directory: string, generation: string) {
  const ownersDirectory = path.join(directory, '.owners', generation);
  if (!fs.existsSync(ownersDirectory)) return false;
  let occupied = false;
  for (const owner of fs.readdirSync(ownersDirectory)) {
    const match = OWNER_PATTERN.exec(owner);
    // Unknown owner records and permission errors are not proof of death.
    if (!match) {
      occupied = true;
      continue;
    }
    try {
      process.kill(Number(match[1]), 0);
      occupied = true;
    } catch (error: unknown) {
      if ((error as NodeJS.ErrnoException).code !== 'ESRCH') {
        occupied = true;
        continue;
      }
      fs.rmSync(path.join(ownersDirectory, owner), { force: true });
    }
  }
  return occupied;
}

export async function pruneDevGenerationCaches(
  root: string,
  activeGeneration: string
) {
  if (!GENERATION_PATTERN.test(activeGeneration))
    throw new Error('Invalid dev generation');
  if (!fs.existsSync(generationsDirectory(root))) return [];
  return withGenerationLock(root, async (directory) => {
    const candidates = fs
      .readdirSync(directory, { withFileTypes: true })
      .filter(
        (entry) => entry.isDirectory() && GENERATION_PATTERN.test(entry.name)
      )
      .map((entry) => ({
        generation: entry.name,
        directory: path.join(directory, entry.name),
        modifiedAt: fs.statSync(path.join(directory, entry.name)).mtimeMs
      }))
      .sort((left, right) => right.modifiedAt - left.modifiedAt);
    const recent = [
      ...new Set([
        activeGeneration,
        ...candidates.map((entry) => entry.generation)
      ])
    ].slice(0, GENERATIONS_RETAINED);
    const retained = new Set(recent);
    const removed: string[] = [];
    for (const candidate of candidates) {
      const occupied = hasLiveOwners(directory, candidate.generation);
      // Reachability-based collection: live owners are roots regardless of age.
      if (occupied || retained.has(candidate.generation)) continue;
      await fs.promises.rm(candidate.directory, {
        recursive: true,
        force: true
      });
      fs.rmSync(path.join(directory, '.owners', candidate.generation), {
        recursive: true,
        force: true
      });
      removed.push(candidate.generation);
    }
    return removed;
  });
}
