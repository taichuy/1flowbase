//! HTTP Request and response header handling.
use headers::Error;
use http::HeaderValue;

mod sec_websocket_extensions;
#[allow(unused)]
pub(crate) use sec_websocket_extensions::{
    SecWebsocketExtensions, WebsocketExtensionParam, WebsocketProtocolExtension,
};

/// Reads a comma-delimited raw header into a Vec.
fn from_comma_delimited<'i, I, T, E>(values: &mut I) -> Result<E, Error>
where
    I: Iterator<Item = &'i HeaderValue>,
    T: ::std::str::FromStr,
    E: ::std::iter::FromIterator<T>,
{
    from_delimited(&mut values.flat_map(|header_value| header_value.to_str()), ',')
}

/// Reads a single-character-delimited raw header into a Vec.
fn from_delimited<'i, I, T, E>(values: &mut I, delimiter: char) -> Result<E, Error>
where
    I: Iterator<Item = &'i str>,
    T: ::std::str::FromStr,
    E: ::std::iter::FromIterator<T>,
{
    values
        .flat_map(|string| {
            let mut in_quotes = false;
            string
                .split(move |c| {
                    #[allow(clippy::collapsible_else_if)]
                    if in_quotes {
                        if c == '"' {
                            in_quotes = false;
                        }
                        false // dont split
                    } else {
                        if c == delimiter {
                            true // split
                        } else {
                            if c == '"' {
                                in_quotes = true;
                            }
                            false // dont split
                        }
                    }
                })
                .filter_map(|x| match x.trim() {
                    "" => None,
                    y => Some(y),
                })
                .map(|x| x.parse().map_err(|_| Error::invalid()))
        })
        .collect()
}
