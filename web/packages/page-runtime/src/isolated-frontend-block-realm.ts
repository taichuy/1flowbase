import {
  isIsolatedFrontendBlockCapability,
  isIsolatedFrontendBlockOutputPublishRequest,
  type IsolatedFrontendBlockCapabilityRequest,
  type IsolatedFrontendBlockCapabilityAck,
  type IsolatedFrontendBlockCapabilityResponse,
  type IsolatedFrontendBlockHostCommand,
  type IsolatedFrontendBlockOutputPublishRequest,
  type IsolatedFrontendBlockProgram,
  type IsolatedFrontendBlockRealmEvent
} from '@1flowbase/page-protocol';

import { sha256Text } from './sha256';

export const ISOLATED_FRONTEND_BLOCK_SANDBOX = 'allow-scripts' as const;

export type IsolatedFrontendBlockRealmState =
  | 'prepared'
  | 'mounting'
  | 'mounted'
  | 'updated'
  | 'failed'
  | 'terminated';

export type IsolatedFrontendBlockCapabilityHandlers = Partial<{
  'block.output.publish': (
    payload: IsolatedFrontendBlockOutputPublishRequest
  ) => void | Promise<void>;
}>;

export interface IsolatedFrontendBlockRealmOptions {
  capabilityHandlers?: IsolatedFrontendBlockCapabilityHandlers;
  onError?(error: Error): void;
}

export interface IsolatedFrontendBlockRealmHandle {
  readonly state: IsolatedFrontendBlockRealmState;
  readonly iframe: HTMLIFrameElement | null;
  mount(root: Element): Promise<void>;
  update(props: Record<string, unknown>): void;
  terminate(): void;
  dispose(): void;
}

const INITIALIZATION_MESSAGE = '1flowbase_isolated_frontend_init';
const MOUNT_TIMEOUT_MS = 5_000;

const ISOLATED_REALM_BOOTSTRAP = `(() => {
  let port = null;
  let instance = null;
  let nextRequestId = 0;
  const pending = new Map();
  const respond = (message) => port && port.postMessage(message);
  const capabilities = Object.freeze({
    request(capability, payload) {
      if (!port) return Promise.reject(new Error('isolated realm is not connected'));
      const requestId = String(++nextRequestId);
      respond({ type: 'capability_request', requestId, capability, payload });
      return new Promise((resolve, reject) => pending.set(requestId, { resolve, reject }));
    }
  });
  const fail = (error) => respond({
    type: 'failed',
    message: error instanceof Error && error.message ? error.message : 'isolated realm failed'
  });
  const receive = async (event) => {
    const message = event.data;
    if (!message || typeof message !== 'object') return;
    if (message.type === 'capability_response') {
      const request = pending.get(message.requestId);
      if (!request) return;
      pending.delete(message.requestId);
      if (message.ok) request.resolve(message.value);
      else request.reject(new Error(message.error || 'capability denied'));
      return;
    }
    try {
      const contract = globalThis.__oneflowbaseIsolatedBlock;
      if (!contract || typeof contract.mount !== 'function') {
        throw new Error('isolated frontend block contract is missing');
      }
      if (message.type === 'mount') {
        instance = (await contract.mount(
          document.getElementById('root'),
          message.props,
          capabilities
        )) || contract;
        respond({ type: 'mounted' });
      } else if (message.type === 'update') {
        if (instance && typeof instance.update === 'function') {
          await instance.update(message.props);
        }
        respond({ type: 'updated' });
      } else if (message.type === 'terminate') {
        if (instance && typeof instance.dispose === 'function') await instance.dispose();
        for (const request of pending.values()) request.reject(new Error('isolated realm terminated'));
        pending.clear();
        port.close();
        port = null;
        instance = null;
      }
    } catch (error) {
      fail(error);
    }
  };
  const initialize = (event) => {
    if (event.source !== parent || event.data?.type !== '${INITIALIZATION_MESSAGE}') return;
    window.removeEventListener('message', initialize);
    port = event.ports[0];
    if (!port) return;
    port.addEventListener('message', receive);
    port.start();
    respond({ type: 'ready' });
  };
  window.addEventListener('message', initialize);
})();`;

export function prepareIsolatedFrontendBlockRealm(
  program: IsolatedFrontendBlockProgram,
  options: IsolatedFrontendBlockRealmOptions = {}
): IsolatedFrontendBlockRealmHandle {
  validateIsolatedFrontendBlockSource(program.source);
  return new BrowserIsolatedFrontendBlockRealm(
    program,
    options.capabilityHandlers ?? {},
    options.onError
  );
}

export async function createIsolatedFrontendBlockSrcdoc(
  source: string
): Promise<string> {
  validateIsolatedFrontendBlockSource(source);
  const [bootstrapHash, programHash] = await Promise.all([
    sha256Base64(ISOLATED_REALM_BOOTSTRAP),
    sha256Base64(source)
  ]);
  const policy = [
    "default-src 'none'",
    `script-src 'sha256-${bootstrapHash}' 'sha256-${programHash}'`,
    "connect-src 'none'",
    "img-src 'none'",
    "media-src 'none'",
    "font-src 'none'",
    "style-src 'unsafe-inline'",
    "object-src 'none'",
    "base-uri 'none'",
    "form-action 'none'",
    "frame-src 'none'",
    "worker-src 'none'",
    "child-src 'none'"
  ].join('; ');
  return [
    '<!doctype html><html><head>',
    `<meta http-equiv="Content-Security-Policy" content="${policy}">`,
    '</head><body><div id="root"></div>',
    `<script>${ISOLATED_REALM_BOOTSTRAP}</script>`,
    `<script>${source}</script>`,
    '</body></html>'
  ].join('');
}

export function validateIsolatedFrontendBlockSource(source: string): void {
  if (!source.trim()) {
    throw new Error('Isolated frontend block source must not be empty.');
  }
  if (
    /<\/script/iu.test(source) ||
    /\b(?:import|export|require|importScripts)\b/u.test(source)
  ) {
    throw new Error(
      'Isolated frontend block source must use the realm contract without imports or exports.'
    );
  }
}

class BrowserIsolatedFrontendBlockRealm implements IsolatedFrontendBlockRealmHandle {
  private realmState: IsolatedFrontendBlockRealmState = 'prepared';
  private frame: HTMLIFrameElement | null = null;
  private channel: MessageChannel | null = null;
  private mountTimeout: ReturnType<typeof setTimeout> | null = null;
  private mountResolve: (() => void) | null = null;
  private mountReject: ((error: Error) => void) | null = null;
  private active = false;

  constructor(
    private readonly program: IsolatedFrontendBlockProgram,
    private readonly handlers: IsolatedFrontendBlockCapabilityHandlers,
    private readonly onError: ((error: Error) => void) | undefined
  ) {}

  get state(): IsolatedFrontendBlockRealmState {
    return this.realmState;
  }

  get iframe(): HTMLIFrameElement | null {
    return this.frame;
  }

  async mount(root: Element): Promise<void> {
    if (this.realmState !== 'prepared') {
      throw new Error(`Cannot mount isolated realm from ${this.realmState}.`);
    }
    if (!(root instanceof Element)) {
      throw new Error('Isolated frontend block root must be a DOM Element.');
    }
    this.realmState = 'mounting';
    try {
      const srcdoc = await createIsolatedFrontendBlockSrcdoc(
        this.program.source
      );
      if (this.hasTerminated()) return;
      const iframe = root.ownerDocument.createElement('iframe');
      iframe.setAttribute('sandbox', ISOLATED_FRONTEND_BLOCK_SANDBOX);
      iframe.setAttribute('srcdoc', srcdoc);
      iframe.setAttribute('referrerpolicy', 'no-referrer');
      iframe.style.border = '0';
      iframe.style.display = 'block';
      iframe.style.width = '100%';
      iframe.style.height = '100%';
      const channel = new MessageChannel();
      this.frame = iframe;
      this.channel = channel;
      this.active = true;
      channel.port1.addEventListener('message', this.receive);
      channel.port1.start();
      iframe.addEventListener('load', this.initialize, { once: true });
      root.replaceChildren(iframe);
      await new Promise<void>((resolve, reject) => {
        this.mountResolve = resolve;
        this.mountReject = reject;
        this.mountTimeout = setTimeout(
          () =>
            this.fail(new Error('Isolated frontend block mount timed out.')),
          MOUNT_TIMEOUT_MS
        );
      });
    } catch (error) {
      if (!this.hasTerminated()) {
        this.realmState = 'failed';
      }
      this.releaseResources();
      throw error;
    }
  }

  update(props: Record<string, unknown>): void {
    if (this.realmState !== 'mounted' && this.realmState !== 'updated') {
      throw new Error(`Cannot update isolated realm from ${this.realmState}.`);
    }
    this.post({ type: 'update', props });
    this.realmState = 'updated';
  }

  terminate(): void {
    if (this.realmState === 'terminated' || this.realmState === 'failed') {
      return;
    }
    this.realmState = 'terminated';
    this.post({ type: 'terminate' });
    const resolveMount = this.mountResolve;
    this.mountResolve = null;
    this.mountReject = null;
    this.releaseResources();
    resolveMount?.();
  }

  dispose(): void {
    this.terminate();
  }

  private readonly initialize = () => {
    const target = this.frame?.contentWindow;
    const transferredPort = this.channel?.port2;
    if (!this.active || !target || !transferredPort) {
      this.fail(new Error('Isolated frontend block iframe is unavailable.'));
      return;
    }
    target.postMessage({ type: INITIALIZATION_MESSAGE }, '*', [
      transferredPort
    ]);
  };

  private readonly receive = (event: MessageEvent<unknown>) => {
    if (!this.active || !isRealmEvent(event.data)) return;
    const message = event.data;
    if (message.type === 'ready') {
      this.post({ type: 'mount', props: this.program.props });
      return;
    }
    if (message.type === 'mounted') {
      this.realmState = 'mounted';
      this.clearMountTimeout();
      this.mountResolve?.();
      this.mountResolve = null;
      this.mountReject = null;
      return;
    }
    if (message.type === 'updated') return;
    if (message.type === 'failed') {
      this.fail(new Error(message.message));
      return;
    }
    void this.dispatchCapability(message);
  };

  private async dispatchCapability(
    request: IsolatedFrontendBlockCapabilityRequest
  ): Promise<void> {
    if (!isIsolatedFrontendBlockCapability(request.capability)) {
      this.respondCapability(
        request.requestId,
        false,
        undefined,
        'capability_denied'
      );
      return;
    }
    if (!isIsolatedFrontendBlockOutputPublishRequest(request.payload)) {
      this.respondCapability(
        request.requestId,
        false,
        undefined,
        'capability_denied'
      );
      return;
    }
    const handler = this.handlers['block.output.publish'];
    if (!handler) {
      this.respondCapability(
        request.requestId,
        false,
        undefined,
        'capability_denied'
      );
      return;
    }
    try {
      await handler(request.payload);
      if (this.active) {
        this.respondCapability(request.requestId, true, { accepted: true });
      }
    } catch {
      if (this.active) {
        this.respondCapability(
          request.requestId,
          false,
          undefined,
          'capability_failed'
        );
      }
    }
  }

  private respondCapability(
    requestId: string,
    ok: boolean,
    value?: IsolatedFrontendBlockCapabilityAck,
    error?: IsolatedFrontendBlockCapabilityResponse['error']
  ): void {
    this.post({
      type: 'capability_response',
      requestId,
      ok,
      ...(ok ? { value } : { error })
    });
  }

  private post(message: IsolatedFrontendBlockHostCommand): void {
    if (!this.active) return;
    this.channel?.port1.postMessage(message);
  }

  private fail(error: Error): void {
    if (!this.active) return;
    this.realmState = 'failed';
    const rejectMount = this.mountReject;
    this.mountResolve = null;
    this.mountReject = null;
    this.releaseResources();
    rejectMount?.(error);
    this.onError?.(error);
  }

  private releaseResources(): void {
    this.active = false;
    this.clearMountTimeout();
    this.frame?.removeEventListener('load', this.initialize);
    this.channel?.port1.removeEventListener('message', this.receive);
    this.channel?.port1.close();
    this.channel?.port2.close();
    this.channel = null;
    this.frame?.remove();
    this.frame = null;
  }

  private clearMountTimeout(): void {
    if (this.mountTimeout !== null) clearTimeout(this.mountTimeout);
    this.mountTimeout = null;
  }

  private hasTerminated(): boolean {
    return this.realmState === 'terminated';
  }
}

function isRealmEvent(
  value: unknown
): value is IsolatedFrontendBlockRealmEvent {
  if (!isRecord(value) || typeof value.type !== 'string') return false;
  if (
    value.type === 'ready' ||
    value.type === 'mounted' ||
    value.type === 'updated'
  ) {
    return true;
  }
  if (value.type === 'failed') return typeof value.message === 'string';
  return (
    value.type === 'capability_request' &&
    typeof value.requestId === 'string' &&
    typeof value.capability === 'string'
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function sha256Base64(value: string): string {
  const digest = sha256Text(value);
  let binary = '';
  for (let index = 0; index < digest.length; index += 2) {
    binary += String.fromCharCode(
      Number.parseInt(digest.slice(index, index + 2), 16)
    );
  }
  return btoa(binary);
}
