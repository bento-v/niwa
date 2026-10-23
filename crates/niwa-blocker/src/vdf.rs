//! A small parser for Valve's text KeyValues format (`.vdf`, `.acf`).
//!
//! Only what Steam's library files need: quoted or bare tokens, nested
//! `{ }` blocks, backslash escapes and `//` line comments.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Str(String),
    Map(Vec<(String, Value)>),
}

impl Value {
    /// Looks up a key in a map, ignoring ASCII case as Steam does.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Map(entries) => entries
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v),
            Value::Str(_) => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            Value::Map(_) => None,
        }
    }

    pub fn entries(&self) -> &[(String, Value)] {
        match self {
            Value::Map(entries) => entries,
            Value::Str(_) => &[],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub message: &'static str,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "vdf line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug, PartialEq, Eq)]
enum Token {
    Str(String),
    Open,
    Close,
}

struct Lexer<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    line: usize,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        // Steam writes UTF-8 without a BOM, but tolerate one.
        let input = input.strip_prefix('\u{feff}').unwrap_or(input);
        Lexer {
            chars: input.chars().peekable(),
            line: 1,
        }
    }

    fn err(&self, message: &'static str) -> ParseError {
        ParseError {
            line: self.line,
            message,
        }
    }

    fn next_token(&mut self) -> Result<Option<Token>, ParseError> {
        loop {
            let Some(&c) = self.chars.peek() else {
                return Ok(None);
            };
            match c {
                '\n' => {
                    self.line += 1;
                    self.chars.next();
                }
                c if c.is_whitespace() => {
                    self.chars.next();
                }
                '/' => {
                    self.chars.next();
                    if self.chars.peek() != Some(&'/') {
                        return Err(self.err("unexpected '/'"));
                    }
                    while let Some(&c) = self.chars.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.chars.next();
                    }
                }
                '{' => {
                    self.chars.next();
                    return Ok(Some(Token::Open));
                }
                '}' => {
                    self.chars.next();
                    return Ok(Some(Token::Close));
                }
                '"' => {
                    self.chars.next();
                    return self.quoted().map(Some);
                }
                _ => return Ok(Some(self.bare())),
            }
        }
    }

    fn quoted(&mut self) -> Result<Token, ParseError> {
        let mut out = String::new();
        loop {
            match self.chars.next() {
                None => return Err(self.err("unterminated string")),
                Some('"') => return Ok(Token::Str(out)),
                Some('\\') => match self.chars.next() {
                    None => return Err(self.err("unterminated string")),
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some(c @ ('\\' | '"')) => out.push(c),
                    // Unknown escapes are kept as written.
                    Some(c) => {
                        out.push('\\');
                        out.push(c);
                    }
                },
                Some(c) => {
                    if c == '\n' {
                        self.line += 1;
                    }
                    out.push(c);
                }
            }
        }
    }

    fn bare(&mut self) -> Token {
        let mut out = String::new();
        while let Some(&c) = self.chars.peek() {
            if c.is_whitespace() || matches!(c, '{' | '}' | '"') {
                break;
            }
            out.push(c);
            self.chars.next();
        }
        Token::Str(out)
    }
}

/// Parses a KeyValues document. The result is always a `Value::Map` of the
/// top-level keys (Steam files have exactly one).
pub fn parse(input: &str) -> Result<Value, ParseError> {
    let mut lexer = Lexer::new(input);
    let entries = parse_entries(&mut lexer, false)?;
    Ok(Value::Map(entries))
}

fn parse_entries(lexer: &mut Lexer<'_>, nested: bool) -> Result<Vec<(String, Value)>, ParseError> {
    let mut entries = Vec::new();
    loop {
        let key = match lexer.next_token()? {
            None if nested => return Err(lexer.err("missing '}'")),
            None => return Ok(entries),
            Some(Token::Close) if nested => return Ok(entries),
            Some(Token::Close) => return Err(lexer.err("unexpected '}'")),
            Some(Token::Open) => return Err(lexer.err("expected a key before '{'")),
            Some(Token::Str(key)) => key,
        };
        let value = match lexer.next_token()? {
            Some(Token::Str(s)) => Value::Str(s),
            Some(Token::Open) => Value::Map(parse_entries(lexer, true)?),
            Some(Token::Close) | None => return Err(lexer.err("key has no value")),
        };
        entries.push((key, value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_blocks_and_escapes() {
        let doc = parse(
            "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t\t\"apps\"\n\t\t{\n\t\t\t\"322170\"\t\t\"328394076\"\n\t\t}\n\t}\n}\n",
        )
        .unwrap();
        let lib = doc.get("libraryfolders").unwrap().get("0").unwrap();
        assert_eq!(
            lib.get("path").unwrap().as_str(),
            Some(r"C:\Program Files (x86)\Steam")
        );
        assert_eq!(
            lib.get("apps").unwrap().get("322170").unwrap().as_str(),
            Some("328394076")
        );
    }

    #[test]
    fn keys_are_case_insensitive() {
        let doc = parse(r#""AppState" { "installdir" "Geometry Dash" }"#).unwrap();
        let state = doc.get("appstate").unwrap();
        assert_eq!(state.get("InstallDir").unwrap().as_str(), Some("Geometry Dash"));
    }

    #[test]
    fn skips_comments_and_reads_bare_tokens() {
        let doc = parse("// header\nroot { key value // trailing\n }").unwrap();
        assert_eq!(doc.get("root").unwrap().get("key").unwrap().as_str(), Some("value"));
    }

    #[test]
    fn reports_unbalanced_braces() {
        assert!(parse(r#""a" { "b" "c""#).is_err());
        assert!(parse(r#""a" "b" }"#).is_err());
        assert!(parse(r#""a" { "b" }"#).is_err());
    }

    #[test]
    fn reports_unterminated_string() {
        let err = parse("\"a\"\n\"oops").unwrap_err();
        assert_eq!(err.line, 2);
    }
}
