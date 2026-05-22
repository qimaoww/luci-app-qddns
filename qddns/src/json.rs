use std::collections::BTreeMap;

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<JsonValue>),
    Object(BTreeMap<String, JsonValue>),
}

impl JsonValue {
    pub fn as_object(&self) -> Option<&BTreeMap<String, JsonValue>> {
        match self {
            Self::Object(map) => Some(map),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[JsonValue]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Number(value) => value.parse::<u64>().ok(),
            _ => None,
        }
    }
}

pub fn parse(input: &str) -> Result<JsonValue> {
    let mut parser = Parser::new(input);
    let value = parser.parse_value()?;
    parser.skip_ws();
    if !parser.is_eof() {
        return Err(Error::new("unexpected trailing JSON data"));
    }
    Ok(value)
}

pub fn stringify(value: &JsonValue) -> String {
    match value {
        JsonValue::Null => "null".into(),
        JsonValue::Bool(value) => {
            if *value {
                "true".into()
            } else {
                "false".into()
            }
        }
        JsonValue::Number(value) => value.clone(),
        JsonValue::String(value) => format!("\"{}\"", escape_string(value)),
        JsonValue::Array(items) => {
            let joined = items.iter().map(stringify).collect::<Vec<_>>().join(",");
            format!("[{joined}]")
        }
        JsonValue::Object(map) => {
            let joined = map
                .iter()
                .map(|(key, value)| format!("\"{}\":{}", escape_string(key), stringify(value)))
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{joined}}}")
        }
    }
}

pub fn pointer<'a>(value: &'a JsonValue, path: &str) -> Option<&'a JsonValue> {
    if path.is_empty() {
        return Some(value);
    }
    if !path.starts_with('/') {
        return None;
    }

    let mut current = value;
    for raw in path.split('/').skip(1) {
        let token = raw.replace("~1", "/").replace("~0", "~");
        current = match current {
            JsonValue::Object(map) => map.get(&token)?,
            JsonValue::Array(items) => {
                let idx = token.parse::<usize>().ok()?;
                items.get(idx)?
            }
            _ => return None,
        };
    }
    Some(current)
}

pub fn escape_string(input: &str) -> String {
    let mut escaped = String::new();
    for ch in input.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\u{08}' => escaped.push_str("\\b"),
            '\u{0c}' => escaped.push_str("\\f"),
            ch if ch.is_control() => {
                escaped.push_str(&format!("\\u{:04x}", ch as u32));
            }
            ch => escaped.push(ch),
        }
    }
    escaped
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn parse_value(&mut self) -> Result<JsonValue> {
        self.skip_ws();
        match self.peek_char() {
            Some('{') => self.parse_object(),
            Some('[') => self.parse_array(),
            Some('"') => Ok(JsonValue::String(self.parse_string()?)),
            Some('t') => {
                self.expect_literal("true")?;
                Ok(JsonValue::Bool(true))
            }
            Some('f') => {
                self.expect_literal("false")?;
                Ok(JsonValue::Bool(false))
            }
            Some('n') => {
                self.expect_literal("null")?;
                Ok(JsonValue::Null)
            }
            Some('-') | Some('0'..='9') => Ok(JsonValue::Number(self.parse_number()?)),
            Some(other) => Err(Error::new(format!("unexpected JSON character '{other}'"))),
            None => Err(Error::new("unexpected end of JSON input")),
        }
    }

    fn parse_object(&mut self) -> Result<JsonValue> {
        self.expect_char('{')?;
        self.skip_ws();
        let mut map = BTreeMap::new();
        if self.consume_char('}') {
            return Ok(JsonValue::Object(map));
        }

        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.skip_ws();
            self.expect_char(':')?;
            let value = self.parse_value()?;
            map.insert(key, value);
            self.skip_ws();
            if self.consume_char('}') {
                break;
            }
            self.expect_char(',')?;
        }

        Ok(JsonValue::Object(map))
    }

    fn parse_array(&mut self) -> Result<JsonValue> {
        self.expect_char('[')?;
        self.skip_ws();
        let mut items = Vec::new();
        if self.consume_char(']') {
            return Ok(JsonValue::Array(items));
        }

        loop {
            let value = self.parse_value()?;
            items.push(value);
            self.skip_ws();
            if self.consume_char(']') {
                break;
            }
            self.expect_char(',')?;
        }

        Ok(JsonValue::Array(items))
    }

    fn parse_string(&mut self) -> Result<String> {
        self.expect_char('"')?;
        let mut out = String::new();
        loop {
            let ch = self
                .bump_char()
                .ok_or_else(|| Error::new("unterminated JSON string"))?;
            match ch {
                '"' => break,
                '\\' => {
                    let escaped = self
                        .bump_char()
                        .ok_or_else(|| Error::new("unterminated JSON escape"))?;
                    match escaped {
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        '/' => out.push('/'),
                        'b' => out.push('\u{08}'),
                        'f' => out.push('\u{0c}'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => {
                            let codepoint = self.parse_hex_quad()?;
                            let ch = char::from_u32(codepoint)
                                .ok_or_else(|| Error::new("invalid unicode escape"))?;
                            out.push(ch);
                        }
                        other => {
                            return Err(Error::new(format!(
                                "unsupported JSON escape '\\{other}'"
                            )))
                        }
                    }
                }
                other => out.push(other),
            }
        }
        Ok(out)
    }

    fn parse_hex_quad(&mut self) -> Result<u32> {
        let mut value = 0u32;
        for _ in 0..4 {
            let ch = self
                .bump_char()
                .ok_or_else(|| Error::new("unterminated unicode escape"))?;
            value = (value << 4)
                | ch.to_digit(16)
                    .ok_or_else(|| Error::new("invalid unicode escape digit"))?;
        }
        Ok(value)
    }

    fn parse_number(&mut self) -> Result<String> {
        let start = self.pos;

        self.consume_char('-');
        self.consume_digits();
        if self.consume_char('.') {
            let consumed = self.consume_digits();
            if consumed == 0 {
                return Err(Error::new("invalid JSON number"));
            }
        }

        if matches!(self.peek_char(), Some('e') | Some('E')) {
            self.bump_char();
            if matches!(self.peek_char(), Some('+') | Some('-')) {
                self.bump_char();
            }
            let consumed = self.consume_digits();
            if consumed == 0 {
                return Err(Error::new("invalid JSON number exponent"));
            }
        }

        let slice = &self.input[start..self.pos];
        if slice.is_empty() || slice == "-" {
            return Err(Error::new("invalid JSON number"));
        }
        Ok(slice.to_string())
    }

    fn consume_digits(&mut self) -> usize {
        let mut count = 0;
        while matches!(self.peek_char(), Some('0'..='9')) {
            self.bump_char();
            count += 1;
        }
        count
    }

    fn expect_literal(&mut self, literal: &str) -> Result<()> {
        for expected in literal.chars() {
            let ch = self
                .bump_char()
                .ok_or_else(|| Error::new(format!("expected '{literal}'")))?;
            if ch != expected {
                return Err(Error::new(format!("expected '{literal}'")));
            }
        }
        Ok(())
    }

    fn expect_char(&mut self, expected: char) -> Result<()> {
        match self.bump_char() {
            Some(ch) if ch == expected => Ok(()),
            Some(ch) => Err(Error::new(format!(
                "expected '{expected}', found '{ch}'"
            ))),
            None => Err(Error::new(format!("expected '{expected}'"))),
        }
    }

    fn consume_char(&mut self, expected: char) -> bool {
        if self.peek_char() == Some(expected) {
            self.bump_char();
            true
        } else {
            false
        }
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek_char(), Some(' ' | '\n' | '\r' | '\t')) {
            self.bump_char();
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn bump_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }
}
