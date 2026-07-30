'use strict';

const fs = require('node:fs');
const crypto = require('node:crypto');

class OwnerHttpClient {
  constructor(baseUrl, fetchImpl = globalThis.fetch) {
    this.baseUrl = baseUrl;
    this.fetch = fetchImpl;
    this.cookie = null;
    this.csrf = null;
  }

  async request(pathname, { method = 'GET', body, multipart = false, csrf = false } = {}) {
    const headers = { accept: 'application/json' };
    if (this.cookie) headers.cookie = this.cookie;
    if (csrf) {
      if (!this.csrf) throw new Error('CSRF token is unavailable');
      headers['x-csrf-token'] = this.csrf;
    }
    let encoded = body;
    if (body !== undefined && !multipart) {
      headers['content-type'] = 'application/json';
      encoded = JSON.stringify(body);
    }
    const response = await this.fetch(`${this.baseUrl}${pathname}`, {
      method,
      headers,
      body: encoded,
      signal: AbortSignal.timeout(30_000),
    });
    const raw = await response.text();
    let payload = null;
    if (raw) {
      try {
        payload = JSON.parse(raw);
      } catch {
        throw new Error(`${method} ${pathname} returned invalid JSON (${response.status})`);
      }
    }
    if (!response.ok) {
      const code = payload?.code ? ` ${payload.code}` : '';
      const message = typeof payload?.message === 'string' && payload.message.trim()
        ? `: ${payload.message.trim()}`
        : '';
      throw new Error(`${method} ${pathname} failed (${response.status}${code}${message})`);
    }
    return { response, data: payload?.data ?? payload };
  }

  async signIn(identifier, password) {
    const result = await this.request('/api/public/auth/sign-in', {
      method: 'POST',
      body: { identifier, password },
    });
    const setCookie = result.response.headers.get('set-cookie');
    if (!setCookie || typeof result.data?.csrf_token !== 'string') {
      throw new Error('sign-in response omitted session cookie or CSRF token');
    }
    this.cookie = setCookie.split(';', 1)[0];
    this.csrf = result.data.csrf_token;
    return result.data;
  }

  async uploadPackage(archivePath) {
    const bytes = fs.readFileSync(archivePath);
    const form = new FormData();
    form.append('file', new Blob([bytes]), require('node:path').basename(archivePath));
    const result = await this.request('/api/console/settings/model-providers/plugins/install-upload', {
      method: 'POST', body: form, multipart: true, csrf: true,
    });
    const id = result.data?.installation?.id;
    if (typeof id !== 'string') throw new Error('package upload response omitted installation id');
    return {
      ...result.data,
      archive_sha256: crypto.createHash('sha256').update(bytes).digest('hex'),
    };
  }

  write(pathname, method = 'POST', body) {
    return this.request(pathname, { method, body, csrf: true });
  }

  read(pathname) {
    return this.request(pathname);
  }
}

module.exports = { OwnerHttpClient };
