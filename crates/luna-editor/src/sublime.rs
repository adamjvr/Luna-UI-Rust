// SPDX-License-Identifier: MPL-2.0

use crate::{SyntaxRule, SyntaxStyle, SyntaxTheme};
use luna_core::{CodedError, ErrorCode};
use luna_theme::Rgba8;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Adapter for Sublime Text `.sublime-color-scheme` JSON files.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SublimeColorSchemeAdapter;

impl SublimeColorSchemeAdapter {
    /// Parses one complete Sublime color-scheme document.
    ///
    /// The adapter accepts ordinary JSON plus line and block comments outside strings. It imports
    /// `name`, the common global colors, rule scopes, rule foreground/background colors, and the
    /// `bold`, `italic`, and `underline` font-style flags. Unknown fields remain forward-compatible.
    pub fn parse(source: &str) -> Result<SyntaxTheme, ColorSchemeError> {
        let stripped = strip_json_comments(source)?;
        let mut parser = JsonParser::new(&stripped);
        let root = parser.parse()?;
        let object = root
            .as_object()
            .ok_or(ColorSchemeError::ExpectedObject("root"))?;
        let name = object
            .get("name")
            .and_then(JsonValue::as_string)
            .unwrap_or("Imported Sublime Scheme")
            .to_owned();
        let globals = object
            .get("globals")
            .and_then(JsonValue::as_object)
            .ok_or(ColorSchemeError::MissingField("globals"))?;
        let foreground = color_field(globals, "foreground")?;
        let background = color_field(globals, "background")?;
        let caret = optional_color_field(globals, "caret")?.unwrap_or(foreground);
        let selection = optional_color_field(globals, "selection")?
            .unwrap_or_else(|| foreground.with_alpha(72));
        let mut rules = Vec::new();
        if let Some(rule_values) = object.get("rules").and_then(JsonValue::as_array) {
            for value in rule_values {
                let Some(rule) = value.as_object() else {
                    return Err(ColorSchemeError::ExpectedObject("rules entry"));
                };
                let Some(scope_value) = rule.get("scope") else {
                    continue;
                };
                let selectors = parse_scope_selectors(scope_value)?;
                if selectors.is_empty() {
                    continue;
                }
                let mut style = SyntaxStyle::default();
                style.foreground = optional_color_field(rule, "foreground")?;
                style.background = optional_color_field(rule, "background")?;
                if let Some(font_style) = rule.get("font_style").and_then(JsonValue::as_string) {
                    for flag in font_style.split_ascii_whitespace() {
                        match flag {
                            "bold" => style.bold = true,
                            "italic" => style.italic = true,
                            "underline" => style.underline = true,
                            "" | "normal" => {}
                            _ => {}
                        }
                    }
                }
                rules.push(SyntaxRule::new(selectors, style));
            }
        }
        Ok(SyntaxTheme {
            name,
            background,
            foreground,
            caret,
            selection,
            rules,
        })
    }
}

/// Failure while parsing a Sublime color-scheme document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ColorSchemeError {
    /// JSON was malformed at the supplied byte offset.
    InvalidJson {
        /// Byte offset in the comment-stripped document.
        offset: usize,
        /// Human-readable parser detail.
        message: String,
    },
    /// A required field was absent.
    MissingField(&'static str),
    /// A field had the wrong JSON shape.
    ExpectedObject(&'static str),
    /// A scope field was neither a string nor an array of strings.
    InvalidScope,
    /// A color field was not a supported hexadecimal value.
    InvalidColor {
        /// Field name.
        field: String,
        /// Supplied value.
        value: String,
    },
    /// A block comment was not terminated.
    UnterminatedComment,
}

impl Display for ColorSchemeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson { offset, message } => {
                write!(
                    formatter,
                    "invalid color-scheme JSON at byte {offset}: {message}"
                )
            }
            Self::MissingField(field) => write!(formatter, "missing color-scheme field {field:?}"),
            Self::ExpectedObject(field) => {
                write!(formatter, "color-scheme field {field:?} must be an object")
            }
            Self::InvalidScope => {
                formatter.write_str("color-scheme scope must be a string or string array")
            }
            Self::InvalidColor { field, value } => {
                write!(
                    formatter,
                    "invalid hexadecimal color {value:?} for field {field:?}"
                )
            }
            Self::UnterminatedComment => {
                formatter.write_str("unterminated block comment in color scheme")
            }
        }
    }
}

impl Error for ColorSchemeError {}
impl CodedError for ColorSchemeError {
    fn error_code(&self) -> ErrorCode {
        ErrorCode::new(match self {
            Self::InvalidJson { .. } => "editor.color_scheme.invalid_json",
            Self::MissingField(_) => "editor.color_scheme.missing_field",
            Self::ExpectedObject(_) => "editor.color_scheme.expected_object",
            Self::InvalidScope => "editor.color_scheme.invalid_scope",
            Self::InvalidColor { .. } => "editor.color_scheme.invalid_color",
            Self::UnterminatedComment => "editor.color_scheme.unterminated_comment",
        })
    }
}

fn color_field(
    object: &BTreeMap<String, JsonValue>,
    field: &'static str,
) -> Result<Rgba8, ColorSchemeError> {
    optional_color_field(object, field)?.ok_or(ColorSchemeError::MissingField(field))
}

fn optional_color_field(
    object: &BTreeMap<String, JsonValue>,
    field: &'static str,
) -> Result<Option<Rgba8>, ColorSchemeError> {
    let Some(value) = object.get(field) else {
        return Ok(None);
    };
    let Some(value) = value.as_string() else {
        return Err(ColorSchemeError::InvalidColor {
            field: field.to_owned(),
            value: format!("{value:?}"),
        });
    };
    parse_hex_color(value)
        .map(Some)
        .ok_or_else(|| ColorSchemeError::InvalidColor {
            field: field.to_owned(),
            value: value.to_owned(),
        })
}

fn parse_scope_selectors(value: &JsonValue) -> Result<Vec<String>, ColorSchemeError> {
    if let Some(scope) = value.as_string() {
        return Ok(scope
            .split(',')
            .map(str::trim)
            .filter(|scope| !scope.is_empty())
            .map(str::to_owned)
            .collect());
    }
    if let Some(values) = value.as_array() {
        let mut selectors = Vec::new();
        for value in values {
            let Some(scope) = value.as_string() else {
                return Err(ColorSchemeError::InvalidScope);
            };
            selectors.extend(
                scope
                    .split(',')
                    .map(str::trim)
                    .filter(|scope| !scope.is_empty())
                    .map(str::to_owned),
            );
        }
        return Ok(selectors);
    }
    Err(ColorSchemeError::InvalidScope)
}

fn parse_hex_color(value: &str) -> Option<Rgba8> {
    let digits = value.strip_prefix('#')?;
    match digits.len() {
        3 => Some(Rgba8::opaque(
            expand_nibble(parse_nibble(digits.as_bytes()[0])?),
            expand_nibble(parse_nibble(digits.as_bytes()[1])?),
            expand_nibble(parse_nibble(digits.as_bytes()[2])?),
        )),
        4 => Some(Rgba8::new(
            expand_nibble(parse_nibble(digits.as_bytes()[0])?),
            expand_nibble(parse_nibble(digits.as_bytes()[1])?),
            expand_nibble(parse_nibble(digits.as_bytes()[2])?),
            expand_nibble(parse_nibble(digits.as_bytes()[3])?),
        )),
        6 => Some(Rgba8::opaque(
            parse_byte(&digits[0..2])?,
            parse_byte(&digits[2..4])?,
            parse_byte(&digits[4..6])?,
        )),
        8 => Some(Rgba8::new(
            parse_byte(&digits[0..2])?,
            parse_byte(&digits[2..4])?,
            parse_byte(&digits[4..6])?,
            parse_byte(&digits[6..8])?,
        )),
        _ => None,
    }
}

fn parse_byte(value: &str) -> Option<u8> {
    u8::from_str_radix(value, 16).ok()
}

const fn parse_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

const fn expand_nibble(value: u8) -> u8 {
    value.saturating_mul(17)
}

fn strip_json_comments(source: &str) -> Result<String, ColorSchemeError> {
    let mut output = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(character) = chars.next() {
        if in_string {
            output.push(character);
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        if character == '"' {
            in_string = true;
            output.push(character);
            continue;
        }
        if character == '/' {
            match chars.peek().copied() {
                Some('/') => {
                    let _ = chars.next();
                    for next in chars.by_ref() {
                        if next == '\n' {
                            output.push('\n');
                            break;
                        }
                    }
                    continue;
                }
                Some('*') => {
                    let _ = chars.next();
                    let mut previous = '\0';
                    let mut terminated = false;
                    for next in chars.by_ref() {
                        if next == '\n' {
                            output.push('\n');
                        }
                        if previous == '*' && next == '/' {
                            terminated = true;
                            break;
                        }
                        previous = next;
                    }
                    if !terminated {
                        return Err(ColorSchemeError::UnterminatedComment);
                    }
                    continue;
                }
                _ => {}
            }
        }
        output.push(character);
    }
    Ok(output)
}

#[derive(Clone, Debug, PartialEq)]
enum JsonValue {
    Null,
    Boolean,
    Number,
    String(String),
    Array(Vec<JsonValue>),
    Object(BTreeMap<String, JsonValue>),
}

impl JsonValue {
    fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            Self::Null | Self::Boolean | Self::Number | Self::Array(_) | Self::Object(_) => None,
        }
    }

    fn as_array(&self) -> Option<&[Self]> {
        match self {
            Self::Array(value) => Some(value),
            Self::Null | Self::Boolean | Self::Number | Self::String(_) | Self::Object(_) => None,
        }
    }

    fn as_object(&self) -> Option<&BTreeMap<String, Self>> {
        match self {
            Self::Object(value) => Some(value),
            Self::Null | Self::Boolean | Self::Number | Self::String(_) | Self::Array(_) => None,
        }
    }
}

struct JsonParser<'a> {
    source: &'a str,
    offset: usize,
}

impl<'a> JsonParser<'a> {
    fn new(source: &'a str) -> Self {
        Self { source, offset: 0 }
    }

    fn parse(&mut self) -> Result<JsonValue, ColorSchemeError> {
        self.skip_whitespace();
        let value = self.parse_value()?;
        self.skip_whitespace();
        if self.offset != self.source.len() {
            return self.error("trailing data");
        }
        Ok(value)
    }

    fn parse_value(&mut self) -> Result<JsonValue, ColorSchemeError> {
        self.skip_whitespace();
        match self.peek_char() {
            Some('{') => self.parse_object(),
            Some('[') => self.parse_array(),
            Some('"') => self.parse_string().map(JsonValue::String),
            Some('t') => {
                self.consume_literal("true")?;
                Ok(JsonValue::Boolean)
            }
            Some('f') => {
                self.consume_literal("false")?;
                Ok(JsonValue::Boolean)
            }
            Some('n') => {
                self.consume_literal("null")?;
                Ok(JsonValue::Null)
            }
            Some('-' | '0'..='9') => {
                self.parse_number()?;
                Ok(JsonValue::Number)
            }
            Some(character) => self.error(&format!("unexpected character {character:?}")),
            None => self.error("unexpected end of input"),
        }
    }

    fn parse_object(&mut self) -> Result<JsonValue, ColorSchemeError> {
        self.expect_char('{')?;
        let mut object = BTreeMap::new();
        self.skip_whitespace();
        if self.consume_char('}') {
            return Ok(JsonValue::Object(object));
        }
        loop {
            self.skip_whitespace();
            let key = self.parse_string()?;
            self.skip_whitespace();
            self.expect_char(':')?;
            let value = self.parse_value()?;
            object.insert(key, value);
            self.skip_whitespace();
            if self.consume_char('}') {
                break;
            }
            self.expect_char(',')?;
        }
        Ok(JsonValue::Object(object))
    }

    fn parse_array(&mut self) -> Result<JsonValue, ColorSchemeError> {
        self.expect_char('[')?;
        let mut values = Vec::new();
        self.skip_whitespace();
        if self.consume_char(']') {
            return Ok(JsonValue::Array(values));
        }
        loop {
            values.push(self.parse_value()?);
            self.skip_whitespace();
            if self.consume_char(']') {
                break;
            }
            self.expect_char(',')?;
        }
        Ok(JsonValue::Array(values))
    }

    fn parse_string(&mut self) -> Result<String, ColorSchemeError> {
        self.expect_char('"')?;
        let mut value = String::new();
        loop {
            let Some(character) = self.next_char() else {
                return self.error("unterminated string");
            };
            match character {
                '"' => return Ok(value),
                '\\' => {
                    let Some(escaped) = self.next_char() else {
                        return self.error("unterminated escape sequence");
                    };
                    match escaped {
                        '"' | '\\' | '/' => value.push(escaped),
                        'b' => value.push('\u{0008}'),
                        'f' => value.push('\u{000c}'),
                        'n' => value.push('\n'),
                        'r' => value.push('\r'),
                        't' => value.push('\t'),
                        'u' => value.push(self.parse_unicode_escape()?),
                        _ => return self.error("unsupported escape sequence"),
                    }
                }
                character if character.is_control() => {
                    return self.error("control character inside string");
                }
                _ => value.push(character),
            }
        }
    }

    fn parse_unicode_escape(&mut self) -> Result<char, ColorSchemeError> {
        let start = self.offset;
        let end = start.saturating_add(4);
        let Some(digits) = self.source.get(start..end) else {
            return self.error("incomplete unicode escape");
        };
        if !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return self.error("invalid unicode escape");
        }
        self.offset = end;
        let value =
            u32::from_str_radix(digits, 16).map_err(|error| ColorSchemeError::InvalidJson {
                offset: start,
                message: error.to_string(),
            })?;
        char::from_u32(value).ok_or_else(|| ColorSchemeError::InvalidJson {
            offset: start,
            message: "unicode escape is not a scalar value".to_owned(),
        })
    }

    fn parse_number(&mut self) -> Result<(), ColorSchemeError> {
        let start = self.offset;
        let _ = self.consume_char('-');
        self.consume_digits();
        if self.consume_char('.') {
            self.consume_digits();
        }
        if matches!(self.peek_char(), Some('e' | 'E')) {
            let _ = self.next_char();
            if matches!(self.peek_char(), Some('+' | '-')) {
                let _ = self.next_char();
            }
            self.consume_digits();
        }
        if self.offset == start {
            return self.error("invalid number");
        }
        if self.source.get(start..self.offset).is_none() {
            return self.error("number slice was invalid");
        }
        Ok(())
    }

    fn consume_digits(&mut self) {
        while self
            .peek_char()
            .is_some_and(|character| character.is_ascii_digit())
        {
            let _ = self.next_char();
        }
    }

    fn consume_literal(&mut self, literal: &str) -> Result<(), ColorSchemeError> {
        if self
            .source
            .get(self.offset..)
            .is_some_and(|remaining| remaining.starts_with(literal))
        {
            self.offset = self.offset.saturating_add(literal.len());
            Ok(())
        } else {
            self.error(&format!("expected {literal:?}"))
        }
    }

    fn expect_char(&mut self, expected: char) -> Result<(), ColorSchemeError> {
        if self.consume_char(expected) {
            Ok(())
        } else {
            self.error(&format!("expected {expected:?}"))
        }
    }

    fn consume_char(&mut self, expected: char) -> bool {
        if self.peek_char() == Some(expected) {
            let _ = self.next_char();
            true
        } else {
            false
        }
    }

    fn skip_whitespace(&mut self) {
        while self.peek_char().is_some_and(char::is_whitespace) {
            let _ = self.next_char();
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.source.get(self.offset..)?.chars().next()
    }

    fn next_char(&mut self) -> Option<char> {
        let character = self.peek_char()?;
        self.offset = self.offset.saturating_add(character.len_utf8());
        Some(character)
    }

    fn error<T>(&self, message: &str) -> Result<T, ColorSchemeError> {
        Err(ColorSchemeError::InvalidJson {
            offset: self.offset,
            message: message.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SublimeColorSchemeAdapter;
    use luna_theme::Rgba8;
    use std::error::Error;

    #[test]
    fn imports_globals_rules_comments_and_font_styles() -> Result<(), Box<dyn Error>> {
        let source = r##"
        {
          // a legal Sublime-style comment
          "name": "Demo",
          "globals": {
            "background": "#101112",
            "foreground": "#f0f1f2",
            "caret": "#ffffff",
            "selection": "#33445588"
          },
          "rules": [
            {
              "scope": "comment, punctuation.definition.comment",
              "foreground": "#66aa66",
              "font_style": "italic"
            },
            {
              "scope": ["keyword.control", "storage.type"],
              "foreground": "#ff9900",
              "font_style": "bold underline"
            }
          ]
        }
        "##;
        let theme = SublimeColorSchemeAdapter::parse(source)?;
        assert_eq!(theme.name, "Demo");
        assert_eq!(theme.background, Rgba8::opaque(0x10, 0x11, 0x12));
        assert_eq!(theme.selection, Rgba8::new(0x33, 0x44, 0x55, 0x88));
        assert_eq!(theme.rules.len(), 2);
        assert!(theme.rules[0].style.italic);
        assert!(theme.rules[1].style.bold);
        assert!(theme.rules[1].style.underline);
        Ok(())
    }

    #[test]
    fn invalid_color_is_reported() {
        let source = r##"{"globals":{"background":"black","foreground":"#fff"}}"##;
        assert!(SublimeColorSchemeAdapter::parse(source).is_err());
    }
}
