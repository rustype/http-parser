use private::SealedRequestParserState;
use std::{collections::HashMap, marker::PhantomData};
use thiserror::Error;

#[doc(hidden)]
const SPACE: u8 = ' ' as u8;
#[doc(hidden)]
const COLON: u8 = ':' as u8;
#[doc(hidden)]
const CR: u8 = '\r' as u8;
#[doc(hidden)]
const LF: u8 = '\n' as u8;
#[doc(hidden)]
const TAB: u8 = '\t' as u8;

type Result<T> = std::result::Result<T, ParsingError>;

/// The HTTP request structure.
///
/// This structure tries to follow RFC 2616 Section 5 <https://tools.ietf.org/html/rfc2616#section-5>.
/// Bellow you can see the expected request format.
/// ```text
/// Request = Request-Line
///           *(( general-header
///            | request-header
///            | entity-header ) CRLF)
///           CRLF
///           [ message-body ]
/// ```
/// *The implementation may not be complete as it is a work in progress.*
#[derive(Debug)]
pub struct Request<'a> {
    /// The method of the request, it can be one of: `OPTIONS`, `GET`, `HEAD`, `POST`, `PUT`, `DELETE`, `TRACE`, `CONNECT`
    method: &'a str,
    request_uri: &'a str,
    http_version: &'a str,
    header: HashMap<&'a str, &'a str>,
    body: &'a str,
}

impl<'a> Request<'a> {
    /// Create a new `Request`.
    fn new() -> Self {
        Self {
            method: "",
            request_uri: "",
            http_version: "",
            header: HashMap::new(),
            body: "",
        }
    }
}

/// The provides the means of state transition for the parser,
/// it provides a single function `parse`,
/// when called it is supposed to parse the stream until the completion of the current state.
pub trait Parse {
    /// `NextState` type are of kind `Parser<'a, State>`
    /// Sadly we can't do `type NextParser = Parser<'a, Self::NextState>`
    /// and allow the final user to simply define `type NextState`
    /// until <https://github.com/rust-lang/rust/issues/29661> is resolved.
    type NextState;

    /// Parse the existing content consuming it in the process,
    /// in the end, return the next parser state.
    fn parse(self) -> Self::NextState;
}

/// A trait for the request parser states.
///
/// *This trait is sealed.*
pub trait RequestParserState: SealedRequestParserState {}

#[doc(hidden)]
mod private {
    pub trait SealedRequestParserState {}

    impl SealedRequestParserState for super::RequestLine {}
    impl SealedRequestParserState for super::Header {}
    impl SealedRequestParserState for super::Body {}
}

/// The `Parser` structure.
#[derive(Debug)]
pub struct HttpRequestParser<'a, State>
where
    State: RequestParserState,
{
    packet: &'a str,
    request: Request<'a>,
    state: PhantomData<State>,
}

impl<'a, T> HttpRequestParser<'a, T>
where
    T: RequestParserState,
{
    /// Skip existing spaces (other whitespace is not considered).
    fn skip_spaces(&mut self) {
        let mut curr = 0;
        let bytes = self.packet.as_bytes();
        while curr < bytes.len() && bytes[curr] == SPACE {
            curr += 1;
        }
        self.packet = &self.packet[curr..];
    }

    /// If the next two characters are
    fn skip_crlf(&mut self) {
        let bytes = self.packet.as_bytes();
        if is_crlf(&[bytes[0], bytes[1]]) {
            self.packet = &self.packet[2..];
        }
    }

    fn parse_until_char(&mut self, chr: u8) -> &'a str {
        let mut curr = 0;
        let bytes = self.packet.as_bytes();
        while curr < bytes.len() && bytes[curr] != chr {
            curr += 1;
        }
        let res = &self.packet[..curr];
        self.packet = &self.packet[curr..];
        res
    }
}

/// The `RequestLine`, the parser starting state.
///
/// It is defined in RFC 2616 as follows:
/// ```text
/// Request-Line = Method SP Request-URI SP HTTP-Version CRLF
/// ```
/// Where `SP` is defined as ASCII character 32 and
/// `CRLF` the combination of ASCII characters 13 and 10 (`\r\n`).
#[derive(Debug)]
pub struct RequestLine;

impl RequestParserState for RequestLine {}

impl<'a> HttpRequestParser<'a, RequestLine> {
    pub fn start(packet: &'a str) -> HttpRequestParser<'a, RequestLine> {
        HttpRequestParser {
            packet,
            request: Request::new(),
            state: PhantomData,
        }
    }

    fn parse_method(&mut self) -> Result<()> {
        let mut curr = 0;
        let bytes = self.packet.as_bytes();
        while bytes[curr] != SPACE {
            curr += 1;
        }
        let method = &self.packet[0..curr];
        if !is_valid_method(method) {
            return Err(ParsingError::InvalidMethod(method.to_string()));
        }
        self.request.method = method;
        self.packet = &self.packet[curr + 1..];
        self.skip_spaces();
        Ok(())
    }

    fn parse_request_uri(&mut self) {
        self.request.request_uri = self.parse_until_char(SPACE);
        self.skip_spaces();
    }

    fn parse_version(&mut self) {
        let mut curr = 0;
        let bytes = self.packet.as_bytes();
        while !is_crlf(&[bytes[curr], bytes[curr + 1]]) {
            curr += 1;
        }
        self.request.http_version = &self.packet[..curr];
        self.packet = &self.packet[curr + 2..];
    }
}

impl<'a> Parse for HttpRequestParser<'a, RequestLine> {
    type NextState = Result<HttpRequestParser<'a, Header>>;

    fn parse(mut self) -> Self::NextState {
        self.parse_method()?;
        self.parse_request_uri();
        self.parse_version();
        Ok(HttpRequestParser::<Header> {
            packet: self.packet,
            request: self.request,
            state: PhantomData,
        })
    }
}

/// The `Header` state, this state should be reached *after* the `RequestLine` state.
#[derive(Debug)]
pub struct Header;

impl RequestParserState for Header {}

impl<'a> HttpRequestParser<'a, Header> {
    fn parse_line(&mut self) {
        // Parse the line key
        let mut curr = 0;
        let bytes = self.packet.as_bytes();
        while !is_whitespace(bytes[curr]) && bytes[curr] != COLON {
            curr += 1;
        }
        let key = &self.packet[0..curr];
        self.packet = &self.packet[curr..];

        // Skip the separator which will match the regex `\s*:\s*`
        let mut curr = 0;
        let bytes = self.packet.as_bytes();
        while is_whitespace(bytes[curr]) || bytes[curr] == COLON {
            curr += 1;
        }
        self.packet = &self.packet[curr..];

        // Parse the line value
        let bytes = self.packet.as_bytes();
        while bytes.len() >= 2 && !is_crlf(&[bytes[curr], bytes[curr + 1]]) {
            curr += 1;
        }
        let value = &self.packet[0..curr];
        self.packet = &self.packet[curr + 2..];

        self.request.header.insert(key, value);
    }
}

impl<'a> Parse for HttpRequestParser<'a, Header> {
    type NextState = HttpRequestParser<'a, Body>;

    fn parse(mut self) -> Self::NextState {
        let mut bytes = self.packet.as_bytes();
        while bytes.len() >= 2 && !is_crlf(&[bytes[0], bytes[1]]) {
            self.parse_line();
            bytes = self.packet.as_bytes();
        }
        self.skip_crlf();
        Self::NextState {
            packet: self.packet,
            request: self.request,
            state: PhantomData,
        }
    }
}

/// The `Body` state, this state should be reached *after* the `Header` state.
#[derive(Debug)]
pub struct Body;

impl RequestParserState for Body {}

impl<'a> Parse for HttpRequestParser<'a, Body> {
    type NextState = Request<'a>;

    fn parse(mut self) -> Self::NextState {
        self.request.body = self.packet;
        self.request
    }
}

/// Checks if the given string slice is a valid HTTP method according to
/// IETF RFC 2616 [5.1.1](https://tools.ietf.org/html/rfc2616#section-5.1.1).
///
/// Supported valid methods are:
/// - `OPTIONS`
/// - `GET`
/// - `HEAD`
/// - `POST`
/// - `PUT`
/// - `DELETE`
/// - `TRACE`
/// - `CONNECT`
fn is_valid_method(method: &str) -> bool {
    match method {
        "OPTIONS" | "GET" | "HEAD" | "POST" | "PUT" | "DELETE" | "TRACE" | "CONNECT" => true,
        _ => false,
    }
}

#[derive(Debug, Error)]
pub enum ParsingError {
    #[error("invalid request method {0}")]
    InvalidMethod(String),
}

/// Check if a pair of bytes are CRLF.
#[inline(always)]
fn is_crlf(bytes: &[u8; 2]) -> bool {
    return bytes[0] == CR && bytes[1] == LF;
}

/// Check if a byte is whitespace.
#[inline(always)]
fn is_whitespace(byte: u8) -> bool {
    return byte == SPACE || byte == LF || byte == CR || byte == TAB;
}
