import { readFile } from 'node:fs/promises';
import { promisify } from 'node:util';
import { brotliCompress, constants, gzip } from 'node:zlib';

const brotli = promisify(brotliCompress);
const compressGzip = promisify(gzip);

export function preferredEncoding(header = ''): 'br' | 'gzip' | undefined {
  const qualities = new Map(
    header
      .toLowerCase()
      .split(',')
      .map((part) => {
        const [name, ...parameters] = part.trim().split(';');
        const quality = parameters.find((value) =>
          value.trim().startsWith('q=')
        );
        const q = quality ? Number(quality.trim().slice(2)) : 1;
        return [name, Number.isFinite(q) && q >= 0 && q <= 1 ? q : 0];
      })
  );
  const br = qualities.get('br') ?? qualities.get('*') ?? 0;
  const gz = qualities.get('gzip') ?? qualities.get('*') ?? 0;
  if (br > 0 && br >= gz) return 'br';
  if (gz > 0) return 'gzip';
}

export function createAssetReader() {
  const cache = new Map<string, Promise<Buffer>>();
  return (file: string, encoding?: 'br' | 'gzip') => {
    const key = `${file}:${encoding ?? 'identity'}`;
    let result = cache.get(key);
    if (!result) {
      result = readFile(file).then((source) => {
        if (encoding === 'br') {
          return brotli(source, {
            params: { [constants.BROTLI_PARAM_QUALITY]: 6 }
          });
        }
        return encoding === 'gzip' ? compressGzip(source) : source;
      });
      cache.set(key, result);
      result.catch(() => cache.delete(key));
    }
    return result;
  };
}
