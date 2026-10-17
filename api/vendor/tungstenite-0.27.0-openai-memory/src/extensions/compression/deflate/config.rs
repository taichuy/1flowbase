use std::{num::NonZeroU8, str::FromStr};

use flate2::Compression;
use log::*;
use thiserror::Error;

use crate::extensions::headers::{WebsocketExtensionParam, WebsocketProtocolExtension};
use crate::protocol::Role;

/// Name of the extension as it appears in the Sec-WebSocket-Extensions header.
///
/// Defined by [RFC 7692 Section 7](https://tools.ietf.org/html/rfc7692#section-7)
pub const PER_MESSAGE_DEFLATE: &str = "permessage-deflate";

/// Extension option that determines whether the server should use the LZ77
/// sliding window from a sent frame for the subsequent frame.
///
/// Defined by [RFC 7692 Section 7.1.1.1](https://tools.ietf.org/html/rfc7692#section-7.1.1.1)
const SERVER_NO_CONTEXT_TAKEOVER: &str = "server_no_context_takeover";

/// Extension option that determines whether the client should use the LZ77
/// sliding window from a sent frame for the subsequent frame.
///
/// Defined by [RFC 7692 Section 7.1.1.2](https://tools.ietf.org/html/rfc7692#section-7.1.1.2)
const CLIENT_NO_CONTEXT_TAKEOVER: &str = "client_no_context_takeover";

/// Extension option that determines the server's max LZ77 sliding window size
/// when compressing outgoing frames.
///
/// Defined by [RFC 7692 Section 7.1.2.1](https://tools.ietf.org/html/rfc7692#section-7.1.2.1)
const SERVER_MAX_WINDOW_BITS: &str = "server_max_window_bits";

/// Extension option that determines the client's max LZ77 sliding window size
/// when compressing outgoing frames.
///
/// Defined by [RFC 7692 Section 7.1.2.2](https://tools.ietf.org/html/rfc7692#section-7.1.2.2)
const CLIENT_MAX_WINDOW_BITS: &str = "client_max_window_bits";

/// Allowed range of values for a [`SERVER_MAX_WINDOW_BITS`] or [`CLIENT_MAX_WINDOW_BITS`] parameter.
///
/// Defined by RFC 7692 Sections 7.1.2.1 and 7.1.2.2.
const ALLOWED_WINDOW_BITS: std::ops::RangeInclusive<NonZeroU8> =
    unsafe { NonZeroU8::new_unchecked(8)..=NonZeroU8::new_unchecked(15) };

/// The supported range of window bit sizes.
///
/// This subset of [`ALLOWED_WINDOW_BITS`] is the range of sizes that this
/// implementation can support.
pub const SUPPORTED_WINDOW_BITS: std::ops::RangeInclusive<NonZeroU8> =
    unsafe { NonZeroU8::new_unchecked(9) }..=*ALLOWED_WINDOW_BITS.end();

/// Errors from `permessage-deflate` extension negotiation.
#[derive(Copy, Clone, Debug, Error)]
#[cfg_attr(test, derive(PartialEq))]
pub enum NegotiationError {
    /// Invalid `server_max_window_bits` value in a negotiation response.
    #[error("Invalid {SERVER_MAX_WINDOW_BITS} value in a negotiation response: {0}")]
    InvalidServerMaxWindowBitsValue(u8),
    /// Missing `server_max_window_bits` value in a negotiation response.
    #[error("Missing {SERVER_MAX_WINDOW_BITS} value in a negotiation response")]
    MissingServerMaxWindowBitsValue,
    /// Missing `server_no_context_takeover` value in a negotiation response.
    #[error("Missing {SERVER_NO_CONTEXT_TAKEOVER} value in a negotiation response")]
    MissingServerNoContextTakeover,
    /// The `server_max_window_bits` value in a negotiation response is not in [`SUPPORTED_WINDOW_BITS`].
    #[error("Unsupported {SERVER_MAX_WINDOW_BITS} value")]
    UnsupportedServerMaxWindowBitsValue(u8),
    /// The `client_max_window_bits` value in a negotiation response is not in [`SUPPORTED_WINDOW_BITS`].
    #[error("Unsupported {CLIENT_MAX_WINDOW_BITS} value")]
    UnsupportedClientMaxWindowBitsValue(u8),
}

/// Errors from parsing a single parameter in a `permessage-deflate` extension
/// directive.
#[derive(Debug, Error)]
#[cfg_attr(test, derive(PartialEq))]
pub(crate) enum ParameterError {
    /// Unknown parameter in a negotiation response.
    #[error("Unknown parameter in a negotiation response: {0}")]
    UnknownParameter(String),
    /// Duplicate parameter in a negotiation response.
    #[error("Duplicate parameter in a negotiation response: {0}")]
    DuplicateParameter(String),
    /// Parameter has an unexpected or invalid value.
    #[error("Invalid value {value} for parameter {name}")]
    InvalidParameterValue { name: &'static str, value: String },
}

/// Contents of a `permessage-deflate` Per-Message Compression Extension.
///
/// This represents the contents of a valid `permessage-deflate` directive found
/// in a `Sec-WebSocket-Extensions` header. Instances of this type can be
/// produced by parsing a sequence of [`WebsocketExtensionParam`]s with
/// [`PermessageDeflateConfig::parse_params`] or with the [`Default`]
/// implementation. Consuming code can assume the fields here are valid
/// according to [RFC 7692 Section 7].
///
/// [RFC 7692 Section 7]: https://tools.ietf.org/html/rfc7692#section-7
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PermessageDeflateConfig {
    server_no_context_takeover: bool,
    client_no_context_takeover: bool,
    /// The `server_max_window_bits` parameter as described by [RFC 7692 Section 7.1.2.1].
    ///
    /// In a legal extension directive, if this parameter is present, it must
    /// have a value. A `None` value indicates the parameter is not present,
    /// while `Some(b)` indicates it is present with value `b`, where `b` is in
    /// the range [`ALLOWED_WINDOW_BITS`].
    ///
    /// [RFC 7692 Section 7.1.2.1]: https://tools.ietf.org/html/rfc7692#section-7.1.2.1
    server_max_window_bits: Option<NonZeroU8>,
    /// The `client_max_window_bits` parameter as described by [RFC 7692 Section 7.1.2.2].
    ///
    /// In a legal extension directive, if this parameter is present, it is not
    /// required to have a value. A `None` value indicates the parameter is not
    /// present, `Some(None)` indicates the parameter is present without a
    /// value, and `Some(Some(b))` indicates it is present with value `b`, where
    /// `b` is in the range [`ALLOWED_WINDOW_BITS`].
    ///
    /// [RFC 7692 Section 7.1.2.2]: https://tools.ietf.org/html/rfc7692#section-7.1.2.2
    client_max_window_bits: Option<Option<NonZeroU8>>,
}

/// Client/server configuration for `permessage-deflate` support.
///
/// This holds configuration values for a client or server for the
/// `permessage-deflate` extension defined in [RFC 7692 Section 7]. This can be
/// used to produce a negotiation offer or response to one as a
/// [`PermessageDeflateConfig`] that is transmitted to the peer as a
/// [`WebsocketExtensionParam`].
///
/// Clients and servers can use the fields and methods on this type to reduce
/// the maximum usage per connection by reducing the size or lifetime of the
/// context windows used during compression or decompression.
///
/// [RFC 7692 Section 7]: https://tools.ietf.org/html/rfc7692#section-7
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeflateConfig {
    /// How hard to try to compress outgoing data.
    pub compression: Compression,
    /// If set, indicates that server compression of a subsequent message won't
    /// reuse the context window of the previous one.
    pub server_no_context_takeover: bool,
    /// If set, indicates that client compression of a subsequent message won't
    /// reuse the context window of the previous one.
    pub client_no_context_takeover: bool,
    /// Guaranteed to be in the range [`SUPPORTED_WINDOW_BITS`].
    server_max_window_bits: NonZeroU8,
    /// Guaranteed to be in the range [`SUPPORTED_WINDOW_BITS`].
    client_max_window_bits: NonZeroU8,
}

/// Error type returned by [`DeflateConfig::set_max_window_bits`].
#[derive(Copy, Clone, Debug, Error)]
#[error("this implementation supports max window bits in {SUPPORTED_WINDOW_BITS:?}")]
pub struct DeflateInvalidMaxWindowBits;

impl DeflateConfig {
    /// Constructs a new [`DeflateConfig`] with default parameters.
    pub fn new() -> Self {
        Self {
            compression: Compression::default(),
            server_no_context_takeover: false,
            client_no_context_takeover: false,
            server_max_window_bits: *SUPPORTED_WINDOW_BITS.end(),
            client_max_window_bits: *SUPPORTED_WINDOW_BITS.end(),
        }
    }

    #[allow(missing_docs)]
    #[inline]
    pub fn server_max_window_bits(&self) -> NonZeroU8 {
        self.server_max_window_bits
    }

    #[allow(missing_docs)]
    #[inline]
    pub fn client_max_window_bits(&self) -> NonZeroU8 {
        self.client_max_window_bits
    }

    /// Limits the maximum number of window bits used by a peer during compression.
    ///
    /// Sets the size of the sliding window that compressed streams sent by the
    /// given role will use. The `bits` value is the base-2 logarithm of the
    /// window size, and must be in the range [`SUPPORTED_WINDOW_BITS`]. If not,
    /// an error is returned.
    #[inline]
    pub fn set_max_window_bits(
        mut self,
        role: Role,
        bits: u8,
    ) -> Result<Self, DeflateInvalidMaxWindowBits> {
        let which = match role {
            Role::Server => &mut self.server_max_window_bits,
            Role::Client => &mut self.client_max_window_bits,
        };

        let bits = NonZeroU8::new(bits)
            .filter(|bits| SUPPORTED_WINDOW_BITS.contains(bits))
            .ok_or(DeflateInvalidMaxWindowBits)?;

        *which = bits;
        Ok(self)
    }

    /// Sets [`server_no_context_takeover`] or [`client_no_context_takeover`].
    ///
    /// The fields are public; this is just a convenience for builder-style usage.
    ///
    /// [`server_no_context_takeover`]: DeflateConfig::server_no_context_takeover
    /// [`client_no_context_takeover`]: DeflateConfig::client_no_context_takeover
    #[inline]
    pub fn set_no_context_takeover(mut self, role: Role, no_context_takeover: bool) -> Self {
        let which = match role {
            Role::Server => &mut self.server_no_context_takeover,
            Role::Client => &mut self.client_no_context_takeover,
        };
        *which = no_context_takeover;
        self
    }

    /// Produces a [`PermessageDeflateConfig`] to send as a client offer to a server.
    ///
    /// The returned value can be serialized as a [`WebsocketProtocolExtension`]
    /// for inclusion in a [`headers::SecWebsocketExtensions`] header.
    pub fn as_offer(&self) -> PermessageDeflateConfig {
        let Self {
            server_no_context_takeover,
            client_no_context_takeover,
            server_max_window_bits,
            client_max_window_bits,
            compression: _,
        } = *self;

        // From RFC 7692 Section 7.1.2.1:
        //
        //   A client MAY include the "server_max_window_bits" extension
        //   parameter in an extension negotiation offer.
        //
        // ...

        //   By including this parameter in an extension negotiation offer, a
        //   client limits the LZ77 sliding window size that the server will use
        //   to compress messages.  If the peer server uses a small LZ77 sliding
        //   window to compress messages, the client can reduce the memory
        //   needed for the LZ77 sliding window.
        //
        //   A server declines an extension negotiation offer with this
        //   parameter if the server doesn't support it.
        //
        // If the client doesn't need the server to use a smaller window size
        // than the protocol max, then don't include the parameter since not all
        // servers will recognize it, and including it might result in the
        // server rejecting the offer.
        let server_max_window_bits = (server_max_window_bits != *ALLOWED_WINDOW_BITS.end())
            .then_some(server_max_window_bits);

        PermessageDeflateConfig {
            server_no_context_takeover,
            client_no_context_takeover,
            server_max_window_bits,
            client_max_window_bits: Some(
                (client_max_window_bits != *ALLOWED_WINDOW_BITS.end())
                    .then_some(client_max_window_bits),
            ),
        }
    }

    /// Receives a negotiation offer from the client and computes the agreed-upon parameters.
    ///
    /// This should be called on the [`DeflateConfig`] representing the server's
    /// initial configuration with the offered parameters from the client as the
    /// argument. If this method returns `Some`, the resulting `DeflateConfig`
    /// may be used as the "agreed parameters" for the connection, and the
    /// resulting [`PermessageDeflateConfig`] should be transmitted to the
    /// client as the response to the offer.
    ///
    /// Note that this method may need to be called multiple times. Per [RFC 7692 Section 5]:
    ///
    ///   A client may also offer multiple PMCE choices to the server by
    ///   including multiple elements in the "Sec-WebSocket-Extensions" header,
    ///   one for each PMCE offered.  This set of elements MAY include multiple
    ///   PMCEs with the same extension name to offer the possibility to use the
    ///   same algorithm with different configuration parameters.  The order of
    ///   elements is important as it specifies the client's preference.  An
    ///   element preceding another element has higher preference.  It is
    ///   recommended that a server accepts PMCEs with higher preference if the
    ///   server supports them.
    ///
    /// [RFC 7692 Section 5]: https://tools.ietf.org/html/rfc7692#section-5
    pub(crate) fn accept_offer(
        &self,
        client: PermessageDeflateConfig,
    ) -> Option<(DeflateConfig, PermessageDeflateConfig)> {
        // RFC 7692 Section 7:

        //   A server MUST decline an extension negotiation offer for this
        //   extension if any of the following conditions are met:
        //   - The negotiation offer contains an extension parameter not defined for use in an offer.
        //   - The negotiation offer contains an extension parameter with an invalid value.
        //   - The negotiation offer contains multiple extension parameters with the same name.
        //   - The server doesn't support the offered configuration.
        let Self {
            server_no_context_takeover,
            client_no_context_takeover,
            server_max_window_bits,
            client_max_window_bits,
            compression,
        } = *self;

        // From RFC 7692 Section 7.1.1.1:
        //
        //   A server accepts an extension negotiation offer that includes the
        //   "server_no_context_takeover" extension parameter by including the
        //   "server_no_context_takeover" extension parameter in the corresponding
        //   extension negotiation response to send back to the client.  The
        //   "server_no_context_takeover" extension parameter in an extension
        //   negotiation response has no value.
        //
        // ...
        //
        //   A server MAY include the "server_no_context_takeover" extension
        //   parameter in an extension negotiation response even if the extension
        //   negotiation offer being accepted by the extension negotiation
        //   response didn't include the "server_no_context_takeover" extension
        //   parameter.
        let server_no_context_takeover =
            server_no_context_takeover || client.server_no_context_takeover;

        // From RFC 7692 Section 7.1.1.2:
        //
        //   A server MAY include the "client_no_context_takeover" extension
        //   parameter in an extension negotiation response.  If the received
        //   extension negotiation offer includes the
        //   "client_no_context_takeover" extension parameter, the server may
        //   either ignore the parameter or use the parameter to avoid taking
        //   over the LZ77 sliding window unnecessarily by including the
        //   "client_no_context_takeover" extension parameter in the
        //   corresponding extension negotiation response to the offer.
        let client_no_context_takeover =
            client_no_context_takeover || client.client_no_context_takeover;

        // From RFC 7692 Section 7.1.2.1:
        //   A server accepts an extension negotiation offer with this parameter
        //   by including the "server_max_window_bits" extension parameter in
        //   the extension negotiation response to send back to the client with
        //   the same or smaller value as the offer.
        //
        //   A server MAY include the "server_max_window_bits" extension
        //   parameter in an extension negotiation response even if the
        //   extension negotiation offer being accepted by the response didn't
        //   include the "server_max_window_bits" extension parameter.
        let (server_max_window_bits, response_server_max_window_bits) = match client
            .server_max_window_bits
        {
            None => (server_max_window_bits, None),
            Some(requested_max) => {
                // Decline the offer if the client is requesting a window that
                // is smaller than we can support.
                if !SUPPORTED_WINDOW_BITS.contains(&requested_max) {
                    debug!("declining offer: {SERVER_MAX_WINDOW_BITS} is smaller than can be supported");
                    return None;
                }
                // It's fine if the client indicated support for a larger window
                // that we can provide; we just downgrade that to our max.
                let bits = requested_max.min(server_max_window_bits);
                (bits, Some(bits))
            }
        };

        let client_max_window_bits = match client.client_max_window_bits {
            None => {
                // From RFC 7692 Section 7.1.2.2:
                //
                //    If a received extension negotiation offer doesn't have the
                //    "client_max_window_bits" extension parameter, the
                //    corresponding extension negotiation response to the offer
                //    MUST NOT include the "client_max_window_bits" extension
                //    parameter.

                if client_max_window_bits != *ALLOWED_WINDOW_BITS.end() {
                    // The server is configured to allocate a limited size
                    // window for each client stream, and the client didn't
                    // indicate that it supports the parameter. Per the RFC,
                    // the server can't include the extension parameter. We
                    // make the choice that it's more important to respect
                    // the server configuration and so decline the offer.
                    debug!("declining offer without {CLIENT_MAX_WINDOW_BITS} (locally limited to {client_max_window_bits})");
                    return None;
                }
                client_max_window_bits
            }
            Some(None) => {
                // The client supports the parameter so we can use our configured value.
                client_max_window_bits
            }
            Some(Some(client_max)) if !SUPPORTED_WINDOW_BITS.contains(&client_max) => {
                // We can't support a window this small.
                debug!("declining offer: {CLIENT_MAX_WINDOW_BITS} is not in the supported range");
                return None;
            }
            Some(Some(client_max)) => client_max.min(client_max_window_bits),
        };

        let connection_config = DeflateConfig {
            compression,
            server_no_context_takeover,
            client_no_context_takeover,
            server_max_window_bits,
            client_max_window_bits,
        };

        let omit_if_max = |value| (value != *ALLOWED_WINDOW_BITS.end()).then_some(value);

        let offer_response = PermessageDeflateConfig {
            server_no_context_takeover,
            client_no_context_takeover,

            server_max_window_bits: response_server_max_window_bits,
            client_max_window_bits: omit_if_max(client_max_window_bits).map(Some),
        };

        Some((connection_config, offer_response))
    }

    /// Receives a response from the server and checks it against the requested context.
    ///
    /// This should be called on the [`DeflateConfig`] representing the client's
    /// configuration, with the response from the server as the argument. An
    /// `Ok` result will indicate the set of options the client should use for
    /// the remainder of the connection.
    pub(crate) fn accept_response(
        self,
        server: PermessageDeflateConfig,
    ) -> Result<Self, NegotiationError> {
        let Self {
            server_no_context_takeover,
            client_no_context_takeover,
            server_max_window_bits,
            client_max_window_bits,
            compression,
        } = self;

        let server_no_context_takeover =
            if server_no_context_takeover && !server.server_no_context_takeover {
                // The client requested no server takeover but the server didn't
                // agree to that.
                return Err(NegotiationError::MissingServerNoContextTakeover);
            } else {
                server.server_no_context_takeover
            };

        // Per RFC 7.1.1.2:
        //
        //   By including the "client_no_context_takeover" extension parameter
        //   in an extension negotiation response, a server prevents the peer
        //   client from using context takeover.
        let client_no_context_takeover =
            client_no_context_takeover || server.client_no_context_takeover;

        let server_max_window_bits = {
            // Per RFC 7.1.2.1:
            //
            //   A server accepts an extension negotiation offer with this
            //   parameter by including the "server_max_window_bits" extension
            //   parameter in the extension negotiation response to send back to
            //   the client with the same or smaller value as the offer.
            //
            // An accepted offer should include the same or smaller value
            // than the one requested, if any.
            let default_server_max_bits = || {
                (server_max_window_bits == *ALLOWED_WINDOW_BITS.end())
                    .then_some(server_max_window_bits)
            };
            let received = server
                .server_max_window_bits
                .or_else(default_server_max_bits)
                .ok_or(NegotiationError::MissingServerMaxWindowBitsValue)?;

            if received > server_max_window_bits {
                return Err(NegotiationError::InvalidServerMaxWindowBitsValue(received.get()));
            }

            if !SUPPORTED_WINDOW_BITS.contains(&received) {
                return Err(NegotiationError::UnsupportedServerMaxWindowBitsValue(received.get()));
            }

            received
        };

        let client_max_window_bits = match server.client_max_window_bits {
            None => {
                // From RFC 7692 Section 7.1.2.2:
                //
                //   If a received extension negotiation offer has the
                //   "client_max_window_bits" extension parameter, the server
                //   MAY include the "client_max_window_bits" extension
                //   parameter in the corresponding extension negotiation
                //   response to the offer.
                //
                // ...
                //
                //   Absence of this extension parameter in an extension
                //   negotiation response indicates that the server can receive
                //   messages compressed using an LZ77 sliding window of up to
                //   32,768 bytes.
                client_max_window_bits
            }
            Some(None) => {
                // From RFC 7692 Section 7.1.2.2:
                //
                //   If the "client_max_window_bits" extension parameter in a
                //   received extension negotiation offer has a value, the
                //   server may either ignore this value or use this value to
                //   avoid allocating an unnecessarily big LZ77 sliding window
                //   by including the "client_max_window_bits" extension
                //   parameter in the corresponding extension negotiation
                //   response to the offer with a value equal to or smaller than
                //   the received value.
                //
                // It's not completely clear whether the RFC allows the server
                // to send the parameter without a value, but in the spirit of
                // Postel's law, interpret this as the server not constraining
                // the client.
                client_max_window_bits
            }
            Some(Some(received)) => {
                // From RFC 7692 Section 7.1.2.2:
                //
                // By including this extension parameter in an extension
                // negotiation response, a server limits the LZ77 sliding window
                // size that the client uses to compress messages.  This reduces
                // the amount of memory for the decompression context that the
                // server has to reserve for the connection.
                if !SUPPORTED_WINDOW_BITS.contains(&received) {
                    return Err(NegotiationError::UnsupportedClientMaxWindowBitsValue(
                        received.get(),
                    ));
                }

                if received > client_max_window_bits {
                    // The server sent us a larger value back than the one we sent.
                    return Err(NegotiationError::UnsupportedClientMaxWindowBitsValue(
                        received.get(),
                    ));
                }

                client_max_window_bits.min(received)
            }
        };

        // Enforce field invariants.
        debug_assert!(SUPPORTED_WINDOW_BITS.contains(&server_max_window_bits));
        debug_assert!(SUPPORTED_WINDOW_BITS.contains(&client_max_window_bits));

        Ok(Self {
            compression,
            server_no_context_takeover,
            client_no_context_takeover,
            server_max_window_bits,
            client_max_window_bits,
        })
    }
}

impl PermessageDeflateConfig {
    /// Generate the corresponding [`WebsocketProtocolExtension`] value.
    fn as_extension(&self) -> WebsocketProtocolExtension {
        let Self {
            server_no_context_takeover,
            client_no_context_takeover,
            server_max_window_bits,
            client_max_window_bits,
        } = self;

        let context_takeovers = [
            server_no_context_takeover.then_some(SERVER_NO_CONTEXT_TAKEOVER),
            client_no_context_takeover.then_some(CLIENT_NO_CONTEXT_TAKEOVER),
        ]
        .into_iter()
        .flatten();

        let max_window_bits = [
            server_max_window_bits.map(|bits| (SERVER_MAX_WINDOW_BITS, Some(bits.to_string()))),
            client_max_window_bits
                .map(|bits| (CLIENT_MAX_WINDOW_BITS, bits.as_ref().map(ToString::to_string))),
        ]
        .into_iter()
        .flatten();

        WebsocketProtocolExtension::new(
            PER_MESSAGE_DEFLATE,
            context_takeovers
                .zip(std::iter::repeat(None))
                .chain(max_window_bits)
                .map(|(name, value)| WebsocketExtensionParam::new(name, value)),
        )
    }

    /// Parses the extension parameter list for a `Sec-WebSocket-Extensions` header.
    pub(crate) fn parse_params<'p>(
        params: impl IntoIterator<Item = &'p WebsocketExtensionParam>,
    ) -> Result<Self, ParameterError> {
        let mut this = Self {
            server_no_context_takeover: false,
            client_no_context_takeover: false,
            server_max_window_bits: None,
            client_max_window_bits: None,
        };

        fn apply<'a>(
            this: &mut PermessageDeflateConfig,
            param: ParamName,
            value: Option<&'a str>,
        ) -> Result<(), Option<&'a str>> {
            match param {
                ParamName::NoContextTakeover(role) => {
                    if value.is_some() {
                        return Err(value);
                    }
                    *match role {
                        Role::Server => &mut this.server_no_context_takeover,
                        Role::Client => &mut this.client_no_context_takeover,
                    } = true;
                    Ok(())
                }

                ParamName::MaxWindowBits(role) => {
                    let bits = value
                        .map(|bits| {
                            // This is more lenient than the RFC allows in that
                            // it will successfully parse an input with leading
                            // zeros. This is in line with Postel's Law ("be
                            // conservative in what you send, be liberal in what
                            // you accept") and won't affect handling for an
                            // RFC-compliant peer.
                            bits.parse()
                                .ok()
                                .filter(|bits| ALLOWED_WINDOW_BITS.contains(bits))
                                .ok_or(value)
                        })
                        .transpose()?;
                    match role {
                        Role::Server => {
                            // Per RFC 7692 Section 7.1.2.1:
                            //
                            //   A client MAY include the
                            //   "server_max_window_bits" extension parameter in
                            //   an extension negotiation offer.  This parameter
                            //   has a decimal integer value without leading
                            //   zeroes between 8 to 15, inclusive...
                            this.server_max_window_bits = Some(bits.ok_or(value)?);
                        }
                        Role::Client => {
                            // Per RFC 7692 Section 7.1.2.2:
                            //
                            //      A client MAY include the
                            //      "client_max_window_bits" extension parameter
                            //      in an extension negotiation offer.  This
                            //      parameter has no value or a decimal integer
                            //      value without leading zeroes between 8 to 15
                            //      inclusive...
                            this.client_max_window_bits = Some(bits)
                        }
                    };
                    Ok(())
                }
            }
        }

        // Set of seen parameters represented as a bit mask.
        let mut seen_params = 0u8;

        for extension_param in params {
            let (name, value) = (extension_param.name(), extension_param.value());
            let param: ParamName = name.parse()?;

            let seen_flag = 1 << param.ordinal();
            if seen_params & seen_flag != 0 {
                return Err(ParameterError::DuplicateParameter(name.to_string()));
            }

            apply(&mut this, param, value).map_err(|value| {
                ParameterError::InvalidParameterValue {
                    name: param.name(),
                    value: value.unwrap_or_default().to_string(),
                }
            })?;

            seen_params |= seen_flag;
        }

        Ok(this)
    }
}

impl Default for DeflateConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl From<&PermessageDeflateConfig> for WebsocketProtocolExtension {
    fn from(value: &PermessageDeflateConfig) -> Self {
        value.as_extension()
    }
}

impl From<PermessageDeflateConfig> for WebsocketProtocolExtension {
    fn from(value: PermessageDeflateConfig) -> Self {
        value.as_extension()
    }
}

#[derive(Copy, Clone)]
enum ParamName {
    NoContextTakeover(Role),
    MaxWindowBits(Role),
}

impl FromStr for ParamName {
    type Err = ParameterError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            CLIENT_MAX_WINDOW_BITS => Self::MaxWindowBits(Role::Client),
            SERVER_MAX_WINDOW_BITS => Self::MaxWindowBits(Role::Server),
            CLIENT_NO_CONTEXT_TAKEOVER => Self::NoContextTakeover(Role::Client),
            SERVER_NO_CONTEXT_TAKEOVER => Self::NoContextTakeover(Role::Server),
            name => return Err(ParameterError::UnknownParameter(name.to_string())),
        })
    }
}

impl ParamName {
    fn ordinal(&self) -> u8 {
        match self {
            Self::NoContextTakeover(role) => *role as u8,
            Self::MaxWindowBits(role) => 2 + *role as u8,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            ParamName::NoContextTakeover(Role::Server) => SERVER_NO_CONTEXT_TAKEOVER,
            ParamName::NoContextTakeover(Role::Client) => CLIENT_NO_CONTEXT_TAKEOVER,
            ParamName::MaxWindowBits(Role::Server) => SERVER_MAX_WINDOW_BITS,
            ParamName::MaxWindowBits(Role::Client) => CLIENT_MAX_WINDOW_BITS,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::extensions::headers::SecWebsocketExtensions;
    use headers::Header;
    use http::HeaderValue;

    use super::*;

    #[test]
    fn deflate_config_parse_params_valid() {
        assert_eq!(
            PermessageDeflateConfig::parse_params([]),
            Ok(PermessageDeflateConfig::default())
        );
        assert_eq!(
            PermessageDeflateConfig::parse_params(
                WebsocketProtocolExtension::from_str(
                    "permessage-deflate; client_max_window_bits=12; server_no_context_takeover"
                )
                .unwrap()
                .params()
            ),
            Ok(PermessageDeflateConfig {
                client_max_window_bits: Some(Some(12.try_into().unwrap())),
                server_no_context_takeover: true,
                ..Default::default()
            })
        );
    }

    #[test]
    fn deflate_rejects_unknown_parameters() {
        assert_eq!(
            PermessageDeflateConfig::parse_params([&WebsocketExtensionParam::new("unknown", None)]),
            Err(ParameterError::UnknownParameter("unknown".to_owned()))
        );
        assert_eq!(
            PermessageDeflateConfig::parse_params([
                &WebsocketExtensionParam::new("client_max_window_bits", Some("13".to_string())),
                &WebsocketExtensionParam::new("after-valid", None)
            ]),
            Err(ParameterError::UnknownParameter("after-valid".to_owned()))
        )
    }

    #[test]
    fn deflate_rejects_duplicate_parameters() {
        assert_eq!(
            PermessageDeflateConfig::parse_params(
                WebsocketProtocolExtension::from_str(
                    "permessage-deflate; client_max_window_bits=12; server_no_context_takeover; client_max_window_bits=12"
            ).unwrap().params()),
            Err(ParameterError::DuplicateParameter("client_max_window_bits".to_owned())),
        );
    }

    #[test]
    fn deflate_config_minimal_client_offer() {
        let client_config = DeflateConfig::new();

        let mut headers = Vec::with_capacity(1);
        SecWebsocketExtensions::new([client_config.as_offer().as_extension()]).encode(&mut headers);

        assert_eq!(
            headers,
            &[HeaderValue::from_static("permessage-deflate; client_max_window_bits")]
        )
    }

    #[test]
    fn deflate_server_respects_offer_server_no_context_takeover() {
        let server_cfg = DeflateConfig::default();

        let client_offer =
            PermessageDeflateConfig { server_no_context_takeover: true, ..Default::default() };

        assert_eq!(
            server_cfg.accept_offer(client_offer),
            Some((
                DeflateConfig { server_no_context_takeover: true, ..server_cfg },
                PermessageDeflateConfig { server_no_context_takeover: true, ..Default::default() }
            ))
        );
    }

    #[test]
    fn rejects_unsupported_client_max_window_bits_offer() {
        let server_config = DeflateConfig::new();

        // With the default value, the client should be able to say it will use
        // a smaller window size than the default.
        const SMALLER_WINDOW: NonZeroU8 = unsafe { NonZeroU8::new_unchecked(12) };
        assert_eq!(
            server_config.accept_offer(PermessageDeflateConfig {
                client_max_window_bits: Some(Some(SMALLER_WINDOW)),
                ..Default::default()
            }),
            Some((
                DeflateConfig { client_max_window_bits: SMALLER_WINDOW, ..server_config },
                PermessageDeflateConfig {
                    client_max_window_bits: Some(Some(SMALLER_WINDOW)),
                    ..Default::default()
                }
            ))
        );
    }

    #[test]
    fn interop() {
        // These are all mutually compatible, though they might result in
        // negotiated parameters that are not the default.
        const MODIFIERS: &[fn(DeflateConfig) -> DeflateConfig] = &[
            |config| config.set_no_context_takeover(Role::Client, true),
            |config| config.set_no_context_takeover(Role::Server, true),
            |config| config.set_max_window_bits(Role::Client, 12).unwrap(),
            |config| config.set_max_window_bits(Role::Server, 10).unwrap(),
        ];

        fn make_config(selector: u8) -> DeflateConfig {
            MODIFIERS
                .iter()
                .enumerate()
                .filter(|(i, _)| (selector & (1 << i) != 0))
                .fold(DeflateConfig::new(), |config, (_, modifier)| modifier(config))
        }

        for client_selector in 0..(1 << MODIFIERS.len()) {
            let client_config = make_config(client_selector);
            for server_selector in 0..(1 << MODIFIERS.len()) {
                let server_config = make_config(server_selector);

                let offer = client_config.as_offer();
                let (_config, response) = server_config.accept_offer(offer).unzip();

                let response = response.unwrap_or_else(|| {
                    panic!("client: {client_config:?}, server: {server_config:?}, offer: {offer:?}")
                });

                let _accepted = client_config.accept_response(response).unwrap_or_else(|e|
                    panic!("client: {client_config:?}, server: {server_config:?}, offer: {offer:?}, response: {response:?}; error: {e}"));
            }
        }
    }

    #[test]
    fn rejects_unsupported_client_max_window_bits_response() {
        let client_config = DeflateConfig::new();

        assert_eq!(client_config.client_max_window_bits().get(), 15);
        // With the default value, the should be able to say it will use
        // a smaller window size than the default.
        const SMALLER_WINDOW: NonZeroU8 = unsafe { NonZeroU8::new_unchecked(12) };
        let server_response = PermessageDeflateConfig {
            server_max_window_bits: Some(*ALLOWED_WINDOW_BITS.end()),
            client_max_window_bits: Some(Some(SMALLER_WINDOW)),
            ..Default::default()
        };

        assert_eq!(
            client_config.accept_response(server_response),
            Ok(DeflateConfig { client_max_window_bits: SMALLER_WINDOW, ..Default::default() })
        );

        // With a smaller allowed maximum window size, the same response will be rejected.
        let client_config =
            client_config.set_max_window_bits(Role::Client, SMALLER_WINDOW.get() - 1).unwrap();
        assert_eq!(
            client_config.accept_response(server_response),
            Err(NegotiationError::UnsupportedClientMaxWindowBitsValue(SMALLER_WINDOW.get()))
        );
    }

    mod rfc_7692_section_7_1_3_examples {
        use headers::HeaderMap;

        use super::*;

        #[track_caller]
        fn parse_extensions(raw: &[u8]) -> SecWebsocketExtensions {
            let headers: HeaderMap = {
                let mut hbuffer = [httparse::EMPTY_HEADER; 20];

                match httparse::parse_headers(raw, &mut hbuffer).unwrap() {
                    httparse::Status::Partial => panic!("preallocated buffer is too small"),
                    httparse::Status::Complete((_size, hdr)) => hdr
                        .iter()
                        .map(|h| {
                            (
                                http::HeaderName::from_bytes(h.name.as_bytes()).unwrap(),
                                HeaderValue::from_bytes(h.value).unwrap(),
                            )
                        })
                        .collect(),
                }
            };
            SecWebsocketExtensions::decode(
                &mut headers.get_all(SecWebsocketExtensions::name()).iter(),
            )
            .unwrap()
        }

        #[track_caller]
        fn parse_deflates(
            extensions: &SecWebsocketExtensions,
        ) -> impl Iterator<Item = PermessageDeflateConfig> + '_ {
            extensions
                .iter()
                .filter_map(|extension| {
                    (extension.name() == PER_MESSAGE_DEFLATE).then_some(extension.params())
                })
                .map(|params| PermessageDeflateConfig::parse_params(params).unwrap())
        }

        #[test]
        fn simplest() {
            // From RFC 7692 Section 7.1.3:
            //    The simplest "Sec-WebSocket-Extensions" header in a client's
            //    opening handshake to offer use of the "permessage-deflate"
            //    extension looks like this:
            let client_headers = parse_extensions(
                b"\
                Sec-WebSocket-Extensions: permessage-deflate\r\n\r\n",
            );
            let client_offers = parse_deflates(&client_headers);

            // ...
            //
            //    Since the "client_max_window_bits" extension parameter is not
            //    included in this extension negotiation offer, the server must
            //    not accept the offer with an extension negotiation response
            //    that includes the "client_max_window_bits" extension
            //    parameter.  The simplest "Sec- WebSocket-Extensions" header in
            //    a server's opening handshake to accept use of the
            //    "permessage-deflate" extension is the same
            let server_config = DeflateConfig::default();
            let (_server_config, accepted_offer) =
                client_offers.filter_map(|offer| server_config.accept_offer(offer)).next().unwrap();

            assert_eq!(SecWebsocketExtensions::new([accepted_offer.as_extension()]), client_headers)
        }

        #[test]
        fn client_multiple_offers() {
            // From RFC 7692 Section 7.1.3:
            //
            //   The following extension negotiation offer sent by a client is
            //   asking the server to use an LZ77 sliding window with a size of
            //   1,024 bytes or less and declaring that the client supports the
            //   "client_max_window_bits" extension parameter in an extension
            //   negotiation response.
            //
            // ...
            //
            //   This extension negotiation offer might be rejected by the
            //   server because the server doesn't support the
            //   "server_max_window_bits" extension parameter in an extension
            //   negotiation offer.  This is fine if the client cannot receive
            //   messages compressed using a larger sliding window size, but if
            //   the client just prefers using a small window but wants to fall
            //   back to the "permessage-deflate" without the
            //   "server_max_window_bits" extension parameter, the client can
            //   make an offer with the fallback option like this:
            let client_headers = parse_extensions(
                b"Sec-WebSocket-Extensions: \
                  permessage-deflate; \
                  client_max_window_bits; server_max_window_bits=10, \
                  permessage-deflate; \
                  client_max_window_bits\r\n\r\n",
            );

            let client_offers = parse_deflates(&client_headers);

            let server_config = DeflateConfig::default();
            let accepted_offers = client_offers
                .filter_map(|offer| server_config.accept_offer(offer))
                .map(|(_server_config, accepted)| {
                    SecWebsocketExtensions::new([accepted.as_extension()])
                })
                .collect::<Vec<_>>();
            // ...
            //
            //   The server can accept "permessage-deflate" by picking any
            //   supported one from the listed offers.  To accept the first
            //   option, for example, the server may send back a response as
            //   follows:
            assert_eq!(
                accepted_offers,
                [
                    parse_extensions(
                        b"Sec-WebSocket-Extensions: \
                    permessage-deflate; server_max_window_bits=10\r\n\r\n"
                    ),
                    // ...
                    //
                    //    To accept the second option, for example, the server may send
                    //    back a response as follows:
                    parse_extensions(b"Sec-WebSocket-Extensions: permessage-deflate\r\n\r\n")
                ]
            );
        }
    }
}
