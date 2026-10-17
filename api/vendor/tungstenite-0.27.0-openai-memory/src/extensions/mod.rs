//! WebSocket extensions.
// Only `permessage-deflate` is supported at the moment.

use bytes::Bytes;
use thiserror::Error;

use crate::extensions::compression::{
    CompressionError, DecompressionError, PerMessageCompressionContext,
};
#[cfg(feature = "handshake")]
use crate::extensions::headers::{SecWebsocketExtensions, WebsocketProtocolExtension};
use crate::protocol::Role;

pub mod compression;
#[cfg(feature = "headers")]
pub(crate) mod headers;

/// Container for configured extensions for a connection.
#[derive(Debug, Default)]
#[allow(missing_copy_implementations)]
pub struct Extensions {
    /// The Per-Message Compression extension configured for the connection, if
    /// any.
    per_message_compression: Option<PerMessageCompressionContext>,
}

/// Configuration for extensions for a connection.
#[derive(Copy, Clone, Default, Debug)]
#[non_exhaustive]
pub struct ExtensionsConfig {
    /// Configuration for the `permessage-deflate` PMCE as specified by [RFC 7692].
    ///
    /// [RFC 7692]: https://tools.ietf.org/html/rfc7692
    #[cfg(feature = "deflate")]
    pub permessage_deflate: Option<compression::deflate::DeflateConfig>,
}

/// Error encountered while handling extensions.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum ExtensionsError {
    /// The header included an invalid extension.
    #[error("Extension header had invalid extension: {0}")]
    InvalidExtension(Box<str>),
    /// The negotiation response included an extension more than once.
    #[error("Extension negotiation response had conflicting extension: {0}")]
    ExtensionConflict(Box<str>),
    /// The header included an unparsable extension.
    #[error("Extension negotiation response had malformed extension: {0}")]
    MalformedExtension(&'static str),
}

#[cfg(feature = "handshake")]
impl ExtensionsConfig {
    pub(crate) fn generate_offers(&self) -> impl Iterator<Item = WebsocketProtocolExtension> {
        let Self {
            #[cfg(feature = "deflate")]
            permessage_deflate,
        } = self;

        #[allow(unused_mut, unused_assignments)]
        let mut permessage_compression_offer = None;

        #[cfg(feature = "deflate")]
        {
            permessage_compression_offer = permessage_deflate.as_ref().map(|p| p.as_offer().into());
        }

        permessage_compression_offer.into_iter()
    }

    /// Checks that the given extensions are compatible with the given config
    /// (if any).
    ///
    /// Receives a [`SecWebsocketExtensions`] header in a handshake response and
    /// evaluates it against the given local configuration. Returns a new
    /// `Extensions` that can be used for the connection for the completed
    /// handshake.
    pub(crate) fn verify_agreed_on(
        &self,
        agreed: SecWebsocketExtensions,
    ) -> Result<Extensions, ExtensionsError> {
        #[cfg_attr(not(feature = "deflate"), allow(unused_mut))]
        let mut per_message_compression = None;

        for extension in agreed.iter() {
            match extension.name() {
                #[cfg(feature = "deflate")]
                compression::deflate::EXTENSION_NAME => {
                    use compression::deflate::{
                        DeflateContext, DeflateParameterError, PermessageDeflateConfig,
                        EXTENSION_NAME,
                    };

                    // Already had PMCE configured
                    if per_message_compression.is_some() {
                        return Err(ExtensionsError::ExtensionConflict(EXTENSION_NAME.into()));
                    }

                    let deflate = self
                        .permessage_deflate
                        .ok_or_else(|| ExtensionsError::InvalidExtension(EXTENSION_NAME.into()))?;

                    let extension: PermessageDeflateConfig = PermessageDeflateConfig::parse_params(
                        extension.params(),
                    )
                    .map_err(|_: DeflateParameterError| {
                        ExtensionsError::MalformedExtension(EXTENSION_NAME)
                    })?;

                    let deflate_config = deflate.accept_response(extension).map_err(|e| {
                        ExtensionsError::InvalidExtension(format!("{EXTENSION_NAME}: {e}").into())
                    })?;

                    per_message_compression =
                        Some(DeflateContext::new(Role::Client, deflate_config).into());
                }
                name => return Err(ExtensionsError::InvalidExtension(name.into())),
            }
        }

        Ok(Extensions { per_message_compression })
    }

    /// Checks whether the given extension headers are compatible with the given
    /// config (if any).
    ///
    /// Recieves a [`SecWebsocketExtensions`] header in a handshake request and
    /// evaluates it against the given local configuration. Returns a
    /// `SecWebsocketExtensions` header to be sent in the handshake response to
    /// the client, and a `Extensions` value to be used for the connection, once
    /// it is established.
    pub(crate) fn accept_offers(
        &self,
        extensions: &SecWebsocketExtensions,
    ) -> Result<(Extensions, Option<SecWebsocketExtensions>), ExtensionsError> {
        #[cfg_attr(not(feature = "deflate"), allow(unused_mut))]
        let mut per_message_compression = None;

        for extension in extensions.iter() {
            // Only one extension is currently supported. If that changes,
            // this will need to be updated to apply the extensions in the correct order.
            match extension.name() {
                #[cfg(feature = "deflate")]
                compression::deflate::EXTENSION_NAME => {
                    use compression::deflate::{
                        DeflateContext, PermessageDeflateConfig, EXTENSION_NAME,
                    };

                    let deflate = match self.permessage_deflate {
                        Some(deflate) => deflate,
                        None => continue,
                    };

                    let extension = match PermessageDeflateConfig::parse_params(extension.params())
                    {
                        Ok(extension) => extension,
                        Err(e) => {
                            // Per RFC 7692 Section 7:
                            //
                            //  A server MUST decline an extension negotiation
                            //  offer for this extension if any of the following
                            //  conditions are met:
                            //
                            //   o  The negotiation offer contains an extension
                            //   parameter not defined for use in an offer.
                            //
                            // Declining instead of rejecting the request
                            // outright allows clients that conform to a
                            // (currently hypothetical) RFC that supersedes RFC
                            // 7692 to fall back to requesting to the behavior
                            // specified in the latter.
                            log::debug!("{EXTENSION_NAME} extension: {e}");
                            continue;
                        }
                    };
                    // Per RFC 7692 Section 5:
                    //
                    //   A client may also offer multiple PMCE choices to the server
                    //   by including multiple elements in the
                    //   "Sec-WebSocket-Extensions" header, one for each PMCE
                    //   offered.  This set of elements MAY include multiple PMCEs
                    //   with the same extension name to offer the possibility to
                    //   use the same algorithm with different configuration
                    //   parameters.  The order of elements is important as it
                    //   specifies the client's preference.  An element preceding
                    //   another element has higher preference.  It is recommended
                    //   that a server accepts PMCEs with higher preference if the
                    //   server supports them.
                    //
                    // Follow the RFC recommendation by not overwriting a PMCE that
                    // is already configured.
                    if per_message_compression.is_some() {
                        continue;
                    }

                    if let Some((config, response)) = deflate.accept_offer(extension) {
                        per_message_compression = Some((
                            DeflateContext::new(Role::Server, config).into(),
                            response.into(),
                        ));
                    }
                }
                // Ignore any unknown extensions in the offer.
                _ => {}
            }
        }

        let (per_message_compression, response) = match per_message_compression {
            Some((a, b)) => (Some(a), Some(b)),
            None => (None, None),
        };

        Ok((
            Extensions { per_message_compression },
            response.map(|response| SecWebsocketExtensions::new(std::iter::once(response))),
        ))
    }
}

impl ExtensionsConfig {
    /// Bypasses negotiation of extension parameters and enables those that have
    /// been configured.
    ///
    /// Returns an [`Extensions`] that has all the extensions enabled that this
    /// [`ExtensionsConfig`] was configured with.
    pub(crate) fn into_unnegotiated_context(self, role: Role) -> Extensions {
        // This can only be infallible while only one per-message compression
        // extension is supported. If more are added there will need to be some
        // resolution strategy for picking which one takes precedence.
        let Self {
            #[cfg(feature = "deflate")]
            permessage_deflate,
        } = self;

        #[cfg_attr(feature = "deflate", allow(unused_assignments))]
        #[cfg_attr(not(feature = "deflate"), allow(unused_mut))]
        let mut per_message_compression = None;
        #[cfg(feature = "deflate")]
        {
            per_message_compression = permessage_deflate
                .map(|deflate| compression::deflate::DeflateContext::new(role, deflate).into());
        }
        let _ = role;

        Extensions { per_message_compression }
    }
}

impl Extensions {
    /// Returns a function that, if present, compresses a message payload.
    ///
    /// The returned value will only be `Some` if a per-message compression
    /// extension, as specified by [RFC 7692], was configured for the connection
    /// state to which this `Extensions` applies.
    ///
    /// [RFC 7692]: https://tools.ietf.org/html/rfc7692
    #[inline]
    pub(crate) fn per_message_compressor<'s>(
        &'s mut self,
    ) -> Option<impl FnOnce(&Bytes) -> Result<Bytes, CompressionError> + 's> {
        let Self { per_message_compression } = self;

        per_message_compression.as_mut().map(PerMessageCompressionContext::compressor)
    }

    /// Returns a function that, if present, decompresses a frame payload.
    ///
    /// The returned value will only be `Some` if a per-message compression
    /// extension, as specified by [RFC 7692], was configured for the connection
    /// state to which this `Extensions` applies. The closure takes as arguments
    /// the frame payload, in bytes, a boolean indicating whether the frame is
    /// the final one for a message, and the maximum number of uncompressed
    /// bytes to produce before returning an error.
    ///
    /// [RFC 7692]: https://tools.ietf.org/html/rfc7692
    #[inline]
    pub(crate) fn per_message_decompressor<'s>(
        &'s mut self,
    ) -> Option<impl FnMut(&Bytes, bool, usize) -> Result<Bytes, DecompressionError> + 's> {
        let Self { per_message_compression } = self;
        per_message_compression.as_mut().map(PerMessageCompressionContext::decompressor)
    }
}

#[cfg(feature = "handshake")]
#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn accept_offers_ignores_unknown_extensions() {
        let (Extensions { per_message_compression }, response) = ExtensionsConfig::default()
            .accept_offers(&SecWebsocketExtensions::new([
                "unknown-1".parse().unwrap(),
                "other-unknown; a=5; b=3".parse().unwrap(),
            ]))
            .unwrap();

        assert!(matches!(per_message_compression, None));
        assert_eq!(response, None);
    }

    #[test]
    fn accept_offers_with_deflate_disabled() {
        let extensions = ExtensionsConfig::default();

        // With or without #[cfg(feature = "deflate")], the extension should be ignored.
        let (Extensions { per_message_compression }, response) =
            extensions.accept_offers(&SecWebsocketExtensions::new([])).unwrap();

        assert!(matches!(per_message_compression, None));
        assert_eq!(response, None);
    }

    #[cfg(feature = "deflate")]
    #[test]
    fn accept_offers_with_deflate_enabled() {
        let extensions = ExtensionsConfig { permessage_deflate: Some(Default::default()) };

        {
            // If the client doesn't offer permessage-deflate support, the response
            // shouldn't include it.
            let (Extensions { per_message_compression }, response) =
                extensions.accept_offers(&SecWebsocketExtensions::new([])).unwrap();
            assert!(matches!(per_message_compression, None));
            assert_eq!(response, None);
        }

        {
            // If the client does offer support, the response should include it.
            let (Extensions { per_message_compression }, response) = extensions
                .accept_offers(&SecWebsocketExtensions::new([
                    WebsocketProtocolExtension::new(compression::deflate::EXTENSION_NAME, []),
                    WebsocketProtocolExtension::new("some-other-extension", []),
                ]))
                .unwrap();

            assert!(matches!(per_message_compression, Some(_)));
            assert_eq!(
                response,
                Some(SecWebsocketExtensions::new([WebsocketProtocolExtension::new(
                    compression::deflate::EXTENSION_NAME,
                    []
                )]))
            );
        }
    }

    #[cfg(feature = "deflate")]
    #[test]
    fn accept_offers_picks_first_acceptable_offer() {
        use compression::deflate::*;
        let extensions = ExtensionsConfig {
            permessage_deflate: Some(
                DeflateConfig::new().set_max_window_bits(Role::Client, 11).unwrap(),
            ),
        };

        let (Extensions { per_message_compression }, response) = extensions
            .accept_offers(&SecWebsocketExtensions::new([
                // These two offers are declined because they doesn't indicate
                // support for client_max_window_bits, which the server is
                // configured to require.
                "permessage-deflate".parse().unwrap(),
                "permessage-deflate; server_max_window_bits=12".parse().unwrap(),
                // This offer would be acceptable but it has a parameter that the server doesn't recognize.
                "permessage-deflate; client_max_window_bits=11; parameter-from-the-future=3"
                    .parse()
                    .unwrap(),
                // This offer is accepted.
                "permessage-deflate; client_no_context_takeover; client_max_window_bits=11"
                    .parse()
                    .unwrap(),
                // This offer is ignored since an earlier one was accepted.
                "permessage-deflate; client_max_window_bits=10".parse().unwrap(),
            ]))
            .unwrap();

        assert!(matches!(per_message_compression, Some(PerMessageCompressionContext::Deflate(_))));
        assert_eq!(
            response,
            Some(SecWebsocketExtensions::new([DeflateConfig::new()
                .set_no_context_takeover(Role::Client, true)
                .set_max_window_bits(Role::Client, 11)
                .unwrap()
                .as_offer()
                .into()]))
        )
    }

    #[cfg(feature = "deflate")]
    #[test]
    fn verify_agreed_on_deflate_then_garbage() {
        let extensions = ExtensionsConfig { permessage_deflate: Some(Default::default()) };

        let result = extensions.verify_agreed_on(SecWebsocketExtensions::new([
            WebsocketProtocolExtension::new(compression::deflate::EXTENSION_NAME, []),
            WebsocketProtocolExtension::new("unrecognized", []),
        ]));

        assert_eq!(result.unwrap_err(), ExtensionsError::InvalidExtension("unrecognized".into()));
    }

    #[cfg(feature = "deflate")]
    #[test]
    fn verify_agreed_on_deflate_multiple_times() {
        let extensions = ExtensionsConfig { permessage_deflate: Some(Default::default()) };

        let result = extensions.verify_agreed_on(SecWebsocketExtensions::new([
            WebsocketProtocolExtension::new(compression::deflate::EXTENSION_NAME, []),
            WebsocketProtocolExtension::new(
                compression::deflate::EXTENSION_NAME,
                ["client_no_context_takeover".parse().unwrap()],
            ),
        ]));

        assert_eq!(
            result.unwrap_err(),
            ExtensionsError::ExtensionConflict(compression::deflate::EXTENSION_NAME.into())
        );
    }
}
