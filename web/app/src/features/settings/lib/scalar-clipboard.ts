import type { MessageInstance } from 'antd/es/message/interface';
import { i18nText } from '../../../shared/i18n/text';

type FlowseClipboard = Clipboard & {
  __flowbaseScalarPatched__?: boolean;
  __flowbaseOriginalWriteText__?: Clipboard['writeText'];
};

const scalarOperationPathPattern =
  /(?:^|\/)(GET|POST|PUT|PATCH|DELETE|HEAD|OPTIONS|TRACE)\/(.+)$/i;
let activeMessage: MessageInstance | null = null;

export function normalizeScalarClipboardText(text: string): string {
  const hashMatch = text.match(/#(.+)$/);
  const hashContent = hashMatch?.[1];

  if (!hashContent) {
    return text;
  }

  const pathMatch = hashContent.match(scalarOperationPathPattern);
  const copiedPath = pathMatch?.[2];

  if (!copiedPath) {
    return text;
  }

  return copiedPath.startsWith('/') ? copiedPath : `/${copiedPath}`;
}

async function copyTextWithExecCommand(text: string) {
  const textArea = document.createElement('textarea');
  textArea.value = text;
  textArea.style.position = 'fixed';
  textArea.style.left = '-9999px';
  textArea.style.top = '0';
  document.body.appendChild(textArea);
  textArea.focus();
  textArea.select();

  try {
    const successful = document.execCommand('copy');

    if (!successful) {
      throw new Error('Copy command failed');
    }

    activeMessage?.success(i18nText('settings', 'auto.copied') + text);
  } catch (err) {
    activeMessage?.error(i18nText('settings', 'auto.copy_failed_manual'));
    console.error('Copy failed:', err);
    throw err;
  } finally {
    document.body.removeChild(textArea);
  }
}

export function installScalarClipboardPatch(message: MessageInstance) {
  activeMessage = message;

  if (typeof navigator === 'undefined') {
    return () => {
      if (activeMessage === message) activeMessage = null;
    };
  }

  const clipboard = (navigator.clipboard ?? {
    writeText: async (text: string) => copyTextWithExecCommand(text)
  }) as FlowseClipboard;

  if (!navigator.clipboard) {
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: clipboard
    });
  }

  if (clipboard.__flowbaseScalarPatched__) {
    return () => {
      if (activeMessage === message) activeMessage = null;
    };
  }

  const originalWriteText =
    typeof clipboard.writeText === 'function'
      ? clipboard.writeText.bind(clipboard)
      : async (text: string) => copyTextWithExecCommand(text);

  clipboard.writeText = async (text: string) =>
    originalWriteText(normalizeScalarClipboardText(text));
  clipboard.__flowbaseOriginalWriteText__ = originalWriteText;
  clipboard.__flowbaseScalarPatched__ = true;

  return () => {
    if (activeMessage === message) activeMessage = null;
  };
}
