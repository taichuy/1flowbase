import { expect, test } from 'vitest';
import { eventSchemaPointers } from '../workflow-event-schema';

test('discovers declared union and reference fields with RFC6901 escaping', () => {
  expect(
    eventSchemaPointers({
      $defs: {
        business: {
          type: 'object',
          properties: { 'amount/total~value': { type: 'number' } }
        }
      },
      oneOf: [
        { type: 'object', properties: { input: { $ref: '#/$defs/business' } } },
        { type: 'object', properties: { terminal: { enum: ['failed'] } } }
      ]
    })
  ).toEqual([
    '/payload/input',
    '/payload/input/amount~1total~0value',
    '/payload/terminal'
  ]);
});

test('unknown schema contributes no guessed paths and recursive refs terminate', () => {
  expect(eventSchemaPointers(null)).toEqual([]);
  expect(
    eventSchemaPointers({
      properties: {
        child: { $ref: '#' },
        items: {
          type: 'array',
          items: { properties: { name: { type: 'string' } } }
        }
      }
    })
  ).toEqual(['/payload/child', '/payload/items']);
});
