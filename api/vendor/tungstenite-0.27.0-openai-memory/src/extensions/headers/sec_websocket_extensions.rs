use std::{borrow::Cow, fmt::Debug, iter::FromIterator, str::FromStr};

use bytes::BytesMut;
use http::HeaderValue;

use super::{from_comma_delimited, from_delimited};

/// The `Sec-Websocket-Extensions` header.
///
/// This header is used in the Websocket handshake, sent by the client to the
/// server and then from the server to the client. It is a proposed and
/// agreed-upon list of websocket protocol extensions to use.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SecWebsocketExtensions(Vec<WebsocketProtocolExtension>);

/// An extension listed in a [`SecWebsocketExtensions`] header.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WebsocketProtocolExtension {
    name: Cow<'static, str>,
    params: Vec<WebsocketExtensionParam>,
}

/// Named parameter for an extension in a `Sec-Websocket-Extensions` header.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WebsocketExtensionParam {
    name: Cow<'static, str>,
    value: Option<String>,
}

impl SecWebsocketExtensions {
    /// Constructs a new header with the provided extensions.
    pub fn new(extensions: impl IntoIterator<Item = WebsocketProtocolExtension>) -> Self {
        Self(extensions.into_iter().collect())
    }

    /// Returns an iterator over the extensions in this header.
    pub fn iter(&self) -> <&Self as IntoIterator>::IntoIter {
        self.into_iter()
    }

    /// Returns the number of extensions in this header.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns a [`HeaderValue`] with the encoded contents of this header.
    pub fn header_value(&self) -> HeaderValue {
        let extensions = CommaDelimited(self.0.as_slice());
        let mut buffer = BytesMut::with_capacity(extensions.encoded_len());

        extensions.write_with(&mut |slice| buffer.extend_from_slice(slice));

        HeaderValue::from_maybe_shared(buffer).expect("valid construction")
    }
}

impl WebsocketProtocolExtension {
    /// Constructs a new extension directive with the given name and parameters.
    pub fn new(
        name: impl Into<Cow<'static, str>>,
        params: impl IntoIterator<Item = WebsocketExtensionParam>,
    ) -> Self {
        Self { name: name.into(), params: params.into_iter().collect() }
    }

    /// The name of this extension directive.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns an iterator over the parameters for this extension directive.
    pub fn params(&self) -> impl Iterator<Item = &WebsocketExtensionParam> {
        self.params.iter()
    }
}

impl WebsocketExtensionParam {
    /// Constructs a new parameter with the given name and optional value.
    #[inline]
    pub fn new(name: impl Into<Cow<'static, str>>, value: Option<String>) -> Self {
        Self { name: name.into(), value }
    }

    /// The name of the parameter.
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The parameter value, if there is one.
    #[inline]
    pub fn value(&self) -> Option<&str> {
        self.value.as_deref()
    }
}

impl headers::Header for SecWebsocketExtensions {
    fn name() -> &'static ::http::header::HeaderName {
        &::http::header::SEC_WEBSOCKET_EXTENSIONS
    }

    fn decode<'i, I>(values: &mut I) -> Result<Self, headers::Error>
    where
        I: Iterator<Item = &'i HeaderValue>,
    {
        from_comma_delimited(values).map(SecWebsocketExtensions)
    }
    fn encode<E: Extend<headers::HeaderValue>>(&self, values: &mut E) {
        values.extend(std::iter::once(self.header_value()))
    }
}

impl From<WebsocketProtocolExtension> for SecWebsocketExtensions {
    fn from(value: WebsocketProtocolExtension) -> Self {
        Self(vec![value])
    }
}

impl FromIterator<WebsocketProtocolExtension> for SecWebsocketExtensions {
    fn from_iter<T: IntoIterator<Item = WebsocketProtocolExtension>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl IntoIterator for SecWebsocketExtensions {
    type Item = WebsocketProtocolExtension;

    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a SecWebsocketExtensions {
    type Item = &'a WebsocketProtocolExtension;

    type IntoIter = std::slice::Iter<'a, WebsocketProtocolExtension>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl FromStr for WebsocketProtocolExtension {
    type Err = headers::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name, tail) = s.split_once(';').map(|(n, t)| (n, Some(t))).unwrap_or((s, None));

        let params = from_delimited(&mut tail.into_iter(), ';')?;

        Ok(Self { name: name.trim().to_owned().into(), params })
    }
}

impl std::fmt::Display for WebsocketProtocolExtension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { name, params } = self;

        write!(f, "{name}")?;
        for param in params {
            write!(f, "; {param}")?;
        }

        Ok(())
    }
}

impl FromStr for WebsocketExtensionParam {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name, value) = s.split_once('=').map(|(n, t)| (n, Some(t))).unwrap_or((s, None));

        let value = value.map(|value| value.trim().to_owned());

        Ok(Self { name: name.trim().to_owned().into(), value })
    }
}

impl std::fmt::Display for WebsocketExtensionParam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { name, value } = self;

        write!(f, "{name}")?;
        if let Some(value) = value {
            write!(f, "={value}")?;
        }
        Ok(())
    }
}

trait WriteTo {
    fn encoded_len(&self) -> usize {
        let mut size = 0;
        self.write_with(&mut |slice| size += slice.len());
        size
    }

    fn write_with(&self, write: &mut (impl FnMut(&[u8]) + ?Sized));
}

impl WriteTo for WebsocketProtocolExtension {
    fn encoded_len(&self) -> usize {
        let Self { name, params } = self;

        let params_len: usize = params.iter().map(|p| p.encoded_len() + 2).sum();

        name.len() + params_len
    }

    fn write_with(&self, write: &mut (impl FnMut(&[u8]) + ?Sized)) {
        let Self { name, params } = self;
        write(name.as_bytes());

        for param in params {
            write(b"; ");
            param.write_with(write);
        }
    }
}

impl WriteTo for WebsocketExtensionParam {
    fn write_with(&self, write: &mut (impl FnMut(&[u8]) + ?Sized)) {
        let Self { name, value } = self;
        write(name.as_bytes());

        if let Some(value) = value {
            write(b"=");
            write(value.as_bytes());
        }
    }
}

#[derive(Debug)]
struct CommaDelimited<T>(T);

impl<T> CommaDelimited<T> {
    const SEPARATOR: &[u8] = b", ";
}

impl<T: WriteTo> WriteTo for CommaDelimited<&[T]> {
    fn encoded_len(&self) -> usize {
        let all_encoded_len: usize = self.0.iter().map(T::encoded_len).sum();
        let all_separators_len = self.0.len().saturating_sub(1) * Self::SEPARATOR.len();
        all_encoded_len + all_separators_len
    }

    fn write_with(&self, write: &mut (impl FnMut(&[u8]) + ?Sized)) {
        let mut is_first = true;
        for item in self.0 {
            let was_first = std::mem::replace(&mut is_first, false);
            if !was_first {
                write(Self::SEPARATOR);
            }
            item.write_with(write);
        }
    }
}

impl<T: WriteTo, const N: usize> WriteTo for CommaDelimited<[T; N]> {
    fn encoded_len(&self) -> usize {
        CommaDelimited(self.0.as_slice()).encoded_len()
    }

    fn write_with(&self, write: &mut (impl FnMut(&[u8]) + ?Sized)) {
        CommaDelimited(self.0.as_slice()).write_with(write);
    }
}

#[cfg(test)]
mod tests {
    use headers::{Header, HeaderMapExt as _};

    use super::*;

    fn test_decode<T: Header>(values: &[&str]) -> Option<T> {
        let mut map = ::http::HeaderMap::new();
        for val in values {
            map.append(T::name(), val.parse().unwrap());
        }
        map.typed_get()
    }

    #[cfg(test)]
    fn test_encode<T: Header>(header: T) -> ::http::HeaderMap {
        let mut map = ::http::HeaderMap::new();
        map.typed_insert(header);
        map
    }

    #[test]
    fn parse_separate_headers() {
        // From https://tools.ietf.org/html/rfc6455#section-9.1
        let extensions =
            test_decode::<SecWebsocketExtensions>(&["foo", "bar; baz=2"]).expect("valid");

        assert_eq!(
            extensions,
            SecWebsocketExtensions(vec![
                WebsocketProtocolExtension { name: "foo".into(), params: vec![] },
                WebsocketProtocolExtension {
                    name: "bar".into(),
                    params: vec![WebsocketExtensionParam {
                        name: "baz".into(),
                        value: Some("2".to_owned())
                    }],
                }
            ])
        );
    }

    #[test]
    fn round_trip_complex() {
        let extensions = test_decode::<SecWebsocketExtensions>(&[
            "deflate-stream",
            "mux; max-channels=4; flow-control, deflate-stream",
            "private-extension",
        ])
        .expect("valid");

        let headers = test_encode(extensions);
        assert_eq!(
            headers["sec-websocket-extensions"],
            "deflate-stream, mux; max-channels=4; flow-control, deflate-stream, private-extension"
        );
    }

    #[test]
    fn write_to_exact_encoded_len() {
        trait WriteToDyn: Debug {
            fn encoded_len(&self) -> usize;
            fn write_with(&self, write: &mut dyn FnMut(&[u8]));
        }

        impl<W: WriteTo + Debug> WriteToDyn for W {
            fn encoded_len(&self) -> usize {
                WriteTo::encoded_len(self)
            }

            fn write_with(&self, write: &mut dyn FnMut(&[u8])) {
                WriteTo::write_with(self, write);
            }
        }

        // This isn't a required property for correctness but if the length
        // precomputation is wrong we'll over- or under-allocate during
        // conversion.
        let cases: &[Box<dyn WriteToDyn>] = &[
            Box::new(CommaDelimited([
                WebsocketProtocolExtension::from_str("extension-name").unwrap(),
                WebsocketProtocolExtension::from_str("with-params; a=5; b=8").unwrap(),
            ])),
            Box::new(CommaDelimited::<[WebsocketProtocolExtension; 0]>([])),
            Box::new(CommaDelimited([
                WebsocketProtocolExtension::from_str("duplicate-name").unwrap(),
                WebsocketProtocolExtension::from_str("duplicate-name").unwrap(),
                WebsocketProtocolExtension::from_str("duplicate-name").unwrap(),
            ])),
            Box::new(WebsocketProtocolExtension::new(
                "name",
                ["foo=123".parse().unwrap(), "bar".parse().unwrap(), "baz=four".parse().unwrap()],
            )),
        ];

        for case in cases {
            let mut value = Vec::new();
            let expected_len = case.encoded_len();
            case.write_with(&mut |slice| value.extend_from_slice(slice));

            assert_eq!(value.len(), expected_len, "for {case:?}");
        }
    }
}
