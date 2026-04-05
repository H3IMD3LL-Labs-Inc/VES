#[derive(Debug)]
pub enum ParserError {
    InvalidEncoding,
    InvalidFormat,
    UnsupportedFormat,
    EmptyPayload,
}