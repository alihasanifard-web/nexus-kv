// ==============================================================================
// ⚡ NEXUS-KV: REDIS RESP WIRE PROTOCOL (پروتکل ارتباطی استاندارد ردیس)
// Architect: Ali Hasanifard
// ==============================================================================

use bytes::{Buf, BytesMut};
use std::io::{Error, ErrorKind, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RespValue {
    SimpleString(String),
    Error(String),
    Integer(i64),
    BulkString(Option<Vec<u8>>),
    Array(Option<Vec<RespValue>>),
}

pub struct RespCodec;

impl RespCodec {
    /// Parse raw RESP incoming bytes
    pub fn decode(buf: &mut BytesMut) -> Result<Option<RespValue>> {
        if buf.is_empty() {
            return Ok(None);
        }

        match buf[0] {
            b'+' => Self::parse_simple_string(buf),
            b'-' => Self::parse_error(buf),
            b':' => Self::parse_integer(buf),
            b'$' => Self::parse_bulk_string(buf),
            b'*' => Self::parse_array(buf),
            _ => Err(Error::new(ErrorKind::InvalidData, "Unknown RESP type prefix")),
        }
    }

    fn read_line(buf: &mut BytesMut) -> Option<Vec<u8>> {
        for i in 1..buf.len() {
            if buf[i - 1] == b'\r' && buf[i] == b'\n' {
                let line = buf.split_to(i - 1).to_vec();
                buf.advance(2); // Consume \r\n
                return Some(line);
            }
        }
        None
    }

    fn parse_simple_string(buf: &mut BytesMut) -> Result<Option<RespValue>> {
        if let Some(line) = Self::read_line(buf) {
            let s = String::from_utf8(line[1..].to_vec())
                .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
            Ok(Some(RespValue::SimpleString(s)))
        } else {
            Ok(None)
        }
    }

    fn parse_error(buf: &mut BytesMut) -> Result<Option<RespValue>> {
        if let Some(line) = Self::read_line(buf) {
            let s = String::from_utf8(line[1..].to_vec())
                .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
            Ok(Some(RespValue::Error(s)))
        } else {
            Ok(None)
        }
    }

    fn parse_integer(buf: &mut BytesMut) -> Result<Option<RespValue>> {
        if let Some(line) = Self::read_line(buf) {
            let s = std::str::from_utf8(&line[1..])
                .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
            let n = s.parse::<i64>().map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
            Ok(Some(RespValue::Integer(n)))
        } else {
            Ok(None)
        }
    }

    fn parse_bulk_string(buf: &mut BytesMut) -> Result<Option<RespValue>> {
        // Find header \r\n
        let mut crlf_pos = None;
        for i in 1..buf.len() {
            if buf[i - 1] == b'\r' && buf[i] == b'\n' {
                crlf_pos = Some(i - 1);
                break;
            }
        }

        let crlf = match crlf_pos {
            Some(p) => p,
            None => return Ok(None),
        };

        let len_str = std::str::from_utf8(&buf[1..crlf])
            .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
        let len = len_str.parse::<i64>()
            .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;

        if len == -1 {
            buf.advance(crlf + 2);
            return Ok(Some(RespValue::BulkString(None)));
        }

        let total_needed = crlf + 2 + (len as usize) + 2;
        if buf.len() < total_needed {
            return Ok(None); // Need more bytes
        }

        buf.advance(crlf + 2);
        let data = buf.split_to(len as usize).to_vec();
        buf.advance(2); // Consume trailing \r\n
        Ok(Some(RespValue::BulkString(Some(data))))
    }

    fn parse_array(buf: &mut BytesMut) -> Result<Option<RespValue>> {
        let mut crlf_pos = None;
        for i in 1..buf.len() {
            if buf[i - 1] == b'\r' && buf[i] == b'\n' {
                crlf_pos = Some(i - 1);
                break;
            }
        }

        let crlf = match crlf_pos {
            Some(p) => p,
            None => return Ok(None),
        };

        let count_str = std::str::from_utf8(&buf[1..crlf])
            .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
        let count = count_str.parse::<i64>()
            .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;

        if count == -1 {
            buf.advance(crlf + 2);
            return Ok(Some(RespValue::Array(None)));
        }

        let mut temp_buf = buf.clone();
        temp_buf.advance(crlf + 2);

        let mut items = Vec::with_capacity(count as usize);
        for _ in 0..count {
            match Self::decode(&mut temp_buf)? {
                Some(val) => items.push(val),
                None => return Ok(None), // Incomplete frame
            }
        }

        *buf = temp_buf;
        Ok(Some(RespValue::Array(Some(items))))
    }

    /// Encode RespValue to binary wire bytes
    pub fn encode(val: &RespValue) -> Vec<u8> {
        let mut out = Vec::new();
        match val {
            RespValue::SimpleString(s) => {
                out.extend_from_slice(format!("+{}\r\n", s).as_bytes());
            }
            RespValue::Error(e) => {
                out.extend_from_slice(format!("-ERR {}\r\n", e).as_bytes());
            }
            RespValue::Integer(n) => {
                out.extend_from_slice(format!(":{}\r\n", n).as_bytes());
            }
            RespValue::BulkString(Some(bytes)) => {
                out.extend_from_slice(format!("${}\r\n", bytes.len()).as_bytes());
                out.extend_from_slice(bytes);
                out.extend_from_slice(b"\r\n");
            }
            RespValue::BulkString(None) => {
                out.extend_from_slice(b"$-1\r\n");
            }
            RespValue::Array(Some(elements)) => {
                out.extend_from_slice(format!("*{}\r\n", elements.len()).as_bytes());
                for elem in elements {
                    out.extend_from_slice(&Self::encode(elem));
                }
            }
            RespValue::Array(None) => {
                out.extend_from_slice(b"*-1\r\n");
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resp_encoding_and_decoding() {
        // Array: ["SET", "key", "val"]
        let raw = b"*3\r\n$3\r\nSET\r\n$3\r\nkey\r\n$3\r\nval\r\n";
        let mut buf = BytesMut::from(&raw[..]);
        let val = RespCodec::decode(&mut buf).unwrap().unwrap();

        match val {
            RespValue::Array(Some(items)) => {
                assert_eq!(items.len(), 3);
                assert_eq!(items[0], RespValue::BulkString(Some(b"SET".to_vec())));
                assert_eq!(items[1], RespValue::BulkString(Some(b"key".to_vec())));
                assert_eq!(items[2], RespValue::BulkString(Some(b"val".to_vec())));
            }
            _ => panic!("Expected array"),
        }

        // Test encode
        let encoded = RespCodec::encode(&RespValue::SimpleString("OK".to_string()));
        assert_eq!(encoded, b"+OK\r\n");
    }
}
