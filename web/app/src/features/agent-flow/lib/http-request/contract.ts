import type {
  FlowBinding,
  FlowNodeOutputDocument
} from '@1flowbase/flow-schema';

import { i18nText } from '../../../../shared/i18n/text';

const HTTP_REQUEST_DEFAULT_TIMEOUT_MS = 30000;
export const HTTP_REQUEST_RESPONSE_BYTES_STEP = 1024 * 1024;
const HTTP_REQUEST_DEFAULT_MAX_RESPONSE_BYTES =
  6 * HTTP_REQUEST_RESPONSE_BYTES_STEP;
export const HTTP_REQUEST_MAX_RESPONSE_BYTES =
  10 * HTTP_REQUEST_RESPONSE_BYTES_STEP;

export const HTTP_REQUEST_METHOD_OPTIONS = [
  { value: 'GET', label: 'GET' },
  { value: 'POST', label: 'POST' },
  { value: 'PUT', label: 'PUT' },
  { value: 'PATCH', label: 'PATCH' },
  { value: 'DELETE', label: 'DELETE' },
  { value: 'HEAD', label: 'HEAD' },
  { value: 'OPTIONS', label: 'OPTIONS' }
];

export const HTTP_REQUEST_BODY_TYPE_OPTIONS = [
  { value: 'none', label: 'none' },
  { value: 'form-data', label: 'form-data' },
  { value: 'x-www-form-urlencoded', label: 'x-www-form-urlencoded' },
  { value: 'json', label: 'JSON' },
  { value: 'raw', label: 'raw' },
  { value: 'binary', label: 'binary' }
] as const;

export type HttpRequestBodyType =
  (typeof HTTP_REQUEST_BODY_TYPE_OPTIONS)[number]['value'];

export const HTTP_REQUEST_DEFAULT_CONFIG = {
  network_egress: { mode: 'workspace' },
  method: 'GET',
  url: '',
  body_type: 'none',
  verify_ssl: true,
  store_response_as_file: false,
  timeout_ms: HTTP_REQUEST_DEFAULT_TIMEOUT_MS,
  max_response_bytes: HTTP_REQUEST_DEFAULT_MAX_RESPONSE_BYTES
} satisfies Record<string, unknown>;

export const HTTP_REQUEST_DEFAULT_BINDINGS = {
  params: { kind: 'named_bindings', value: [] },
  headers: { kind: 'named_bindings', value: [] },
  body: { kind: 'templated_text', value: '' },
  urlencoded: { kind: 'named_bindings', value: [] },
  form_data: { kind: 'named_bindings', value: [] }
} satisfies Record<string, FlowBinding>;

export const HTTP_REQUEST_OUTPUTS = [
  {
    key: 'body',
    title: i18nText('agentFlow', 'auto.http_response_body'),
    valueType: 'string'
  },
  {
    key: 'status_code',
    title: i18nText('agentFlow', 'auto.http_response_status_code'),
    valueType: 'number'
  },
  {
    key: 'headers',
    title: i18nText('agentFlow', 'auto.http_response_headers_json'),
    valueType: 'object'
  },
  {
    key: 'files',
    title: i18nText('agentFlow', 'auto.http_response_files'),
    valueType: 'Array[File]'
  }
] satisfies FlowNodeOutputDocument[];

export function isHttpRequestBodyType(
  value: unknown
): value is HttpRequestBodyType {
  return HTTP_REQUEST_BODY_TYPE_OPTIONS.some(
    (option) => option.value === value
  );
}
