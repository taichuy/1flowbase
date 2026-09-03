export const WEBMCP_REGISTRATIONS_CHANGED_EVENT =
  '1flowbase:webmcp-registrations-changed';

export function notifyWebMcpRegistrationsChanged() {
  window.dispatchEvent(new Event(WEBMCP_REGISTRATIONS_CHANGED_EVENT));
}
