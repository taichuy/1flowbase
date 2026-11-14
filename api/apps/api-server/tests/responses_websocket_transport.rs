#[macro_use]
#[path = "../../../crates/api/public-gateway/src/test_support_visibility.rs"]
mod test_support_visibility;
// Exercise the production adapter without linking the API-server's database fixtures.
#[path = "../../../crates/api/public-gateway/src/routes/application_public_api/responses_websocket/transport/mod.rs"]
mod transport;

#[path = "../../../crates/api/public-gateway/src/routes/application_public_api/responses_websocket/transport/_tests/mod.rs"]
mod cases;
