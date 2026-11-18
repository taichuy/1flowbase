const { test } = require('node:test');
const assert = require('node:assert/strict');
const { register, registrationBlocks } = require('../core.js');

const source = 'fn registrations() -> Vec<SettingsFeatureRegistration> { vec![] }';
const descriptor = {
  feature_id: 'system.events',
  console_surface: {
    route_id: 'settings.events', surface_key: 'events', path: '/settings/events',
    label_key: 'events.title', description_key: 'events.description', order: 520,
  },
  api_routes: [{ method: 'GET', path: '/api/console/events' }],
};

test('one canonical registration is inserted and replaced without duplicate ownership', () => {
  const added = register(source, '', descriptor);
  assert.equal(registrationBlocks(added, '').length, 1);
  assert.equal(register(added, '', descriptor), added);
  const changed = register(added, '', { ...descriptor, api_routes: [
    ...descriptor.api_routes, { method: 'POST', path: '/api/console/events' },
  ] });
  assert.deepEqual(registrationBlocks(changed, '')[0].routes, [
    'GET /api/console/events', 'POST /api/console/events',
  ]);
});

test('another feature cannot take ownership of an already registered route', () => {
  const added = register(source, '', descriptor);
  assert.throws(() => register(added, '', { ...descriptor, feature_id: 'system.other' }),
    /already belongs to system.events/u);
});

test('route template braces and existing constant identities are preserved', () => {
  const added = register(source, '', { ...descriptor, api_routes: [
    { method: 'PUT', path: '/api/console/events/{event_definition_id}' },
  ] }).replace('feature_id: "system.events".to_string()', 'feature_id: EVENT_V2_ID.to_owned()');
  const contract = 'pub const EVENT_V2_ID: &str = "system.events";';
  assert.equal(registrationBlocks(added, contract)[0].featureId, 'system.events');
  assert.equal(registrationBlocks(register(added, contract, descriptor), contract).length, 1);
});

test('invalid scope and duplicate method/route declarations fail before source mutation', () => {
  assert.throws(() => register(source, '', { ...descriptor, feature_id: 'user.events' }), /namespace/u);
  assert.throws(() => register(source, '', { ...descriptor, api_routes: [
    ...descriptor.api_routes, ...descriptor.api_routes,
  ] }), /Duplicate route/u);
});
