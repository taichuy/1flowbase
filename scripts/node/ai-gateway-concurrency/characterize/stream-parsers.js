'use strict';

function parseSseBlock(block) {
  let event = null;
  const dataLines = [];
  for (const line of block.split('\n')) {
    if (line.startsWith('event:')) event = line.slice(6).trim();
    if (line.startsWith('data:')) dataLines.push(line.slice(5).trimStart());
  }
  if (dataLines.length === 0 || dataLines.join('\n') === '[DONE]') return null;
  return { event, data: JSON.parse(dataLines.join('\n')) };
}

function createSseParser(onEvent) {
  const decoder = new TextDecoder();
  let buffered = '';
  return {
    push(chunk) {
      buffered += decoder.decode(chunk, { stream: true }).replaceAll('\r\n', '\n');
      let boundary = buffered.indexOf('\n\n');
      while (boundary >= 0) {
        const block = buffered.slice(0, boundary);
        buffered = buffered.slice(boundary + 2);
        const parsed = parseSseBlock(block);
        if (parsed) onEvent(parsed);
        boundary = buffered.indexOf('\n\n');
      }
    },
    finish() {
      buffered += decoder.decode().replaceAll('\r\n', '\n');
      if (buffered.trim()) {
        const parsed = parseSseBlock(buffered);
        if (parsed) onEvent(parsed);
      }
      buffered = '';
    },
  };
}

function protocolEventType(parsed) {
  return parsed.event ?? parsed.data?.type ?? null;
}

function eventText(parsed) {
  const data = parsed.data;
  if (data?.type === 'response.output_text.delta') return data.delta;
  if (data?.type === 'content_block_delta' && data.delta?.type === 'text_delta') return data.delta.text;
  return null;
}

function nonceFromText(text) {
  if (typeof text !== 'string') return null;
  return text.match(/\bmock-\d{6}\b/u)?.[0] ?? null;
}

module.exports = { createSseParser, eventText, nonceFromText, parseSseBlock, protocolEventType };
