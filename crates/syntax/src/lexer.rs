//! Lexer for Lutetium source text.
//!
//! Produces a flat token stream. Newlines are real tokens: statements are
//! newline/semicolon separated and blocks are brace delimited (no significant
//! indentation).

use lu_diagnostics::{Diagnostic, Span};

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Int(i64),
    Float(f64),
    Str(String),
    Ident(String),
    KwFn,
    KwLet,
    KwMut,
    KwIf,
    KwElse,
    KwWhile,
    KwFor,
    KwIn,
    KwReturn,
    KwBreak,
    KwContinue,
    KwTrue,
    KwFalse,
    Newline,
    Eof,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Semi,
    DotDot,
    DotDotEq,
    Arrow,
    Dot,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    EqEq,
    NotEq,
    Lt,
    Le,
    Gt,
    Ge,
    AmpAmp,
    PipePipe,
    Bang,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
}

impl Tok {
    /// Human-readable token description for error messages.
    pub fn describe(&self) -> String {
        match self {
            Tok::Int(_) => "integer literal".into(),
            Tok::Float(_) => "float literal".into(),
            Tok::Str(_) => "string literal".into(),
            Tok::Ident(_) => "identifier".into(),
            Tok::KwFn => "'fn'".into(),
            Tok::KwLet => "'let'".into(),
            Tok::KwMut => "'mut'".into(),
            Tok::KwIf => "'if'".into(),
            Tok::KwElse => "'else'".into(),
            Tok::KwWhile => "'while'".into(),
            Tok::KwFor => "'for'".into(),
            Tok::KwIn => "'in'".into(),
            Tok::KwReturn => "'return'".into(),
            Tok::KwBreak => "'break'".into(),
            Tok::KwContinue => "'continue'".into(),
            Tok::KwTrue => "'true'".into(),
            Tok::KwFalse => "'false'".into(),
            Tok::Newline => "end of line".into(),
            Tok::Eof => "end of file".into(),
            Tok::LParen => "'('".into(),
            Tok::RParen => "')'".into(),
            Tok::LBrace => "'{'".into(),
            Tok::RBrace => "'}'".into(),
            Tok::LBracket => "'['".into(),
            Tok::RBracket => "']'".into(),
            Tok::Comma => "','".into(),
            Tok::Colon => "':'".into(),
            Tok::Semi => "';'".into(),
            Tok::Arrow => "'->'".into(),
            Tok::Dot => "'.'".into(),
            Tok::DotDot => "'..'".into(),
            Tok::DotDotEq => "'..='".into(),
            Tok::Plus => "'+'".into(),
            Tok::Minus => "'-'".into(),
            Tok::Star => "'*'".into(),
            Tok::Slash => "'/'".into(),
            Tok::Percent => "'%'".into(),
            Tok::Eq => "'='".into(),
            Tok::EqEq => "'=='".into(),
            Tok::NotEq => "'!='".into(),
            Tok::Lt => "'<'".into(),
            Tok::Le => "'<='".into(),
            Tok::Gt => "'>'".into(),
            Tok::Ge => "'>='".into(),
            Tok::AmpAmp => "'&&'".into(),
            Tok::PipePipe => "'||'".into(),
            Tok::Bang => "'!'".into(),
            Tok::PlusEq => "'+='".into(),
            Tok::MinusEq => "'-='".into(),
            Tok::StarEq => "'*='".into(),
            Tok::SlashEq => "'/='".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub span: Span,
}

fn keyword(s: &str) -> Option<Tok> {
    match s {
        "fn" => Some(Tok::KwFn),
        "let" => Some(Tok::KwLet),
        "mut" => Some(Tok::KwMut),
        "if" => Some(Tok::KwIf),
        "else" => Some(Tok::KwElse),
        "while" => Some(Tok::KwWhile),
        "for" => Some(Tok::KwFor),
        "in" => Some(Tok::KwIn),
        "return" => Some(Tok::KwReturn),
        "break" => Some(Tok::KwBreak),
        "continue" => Some(Tok::KwContinue),
        "true" => Some(Tok::KwTrue),
        "false" => Some(Tok::KwFalse),
        _ => None,
    }
}

/// Lex `src` into tokens. Returns tokens plus lexical diagnostics; tokens are
/// still produced for the recoverable prefix so callers can report several
/// errors in one pass.
pub fn lex(src: &str) -> (Vec<Token>, Vec<Diagnostic>) {
    let chars: Vec<(usize, char)> = src.char_indices().collect();
    let n = chars.len();
    let end = src.len();
    let mut toks: Vec<Token> = Vec::new();
    let mut diags: Vec<Diagnostic> = Vec::new();
    let mut i = 0usize;

    while i < n {
        let (pos, c) = chars[i];
        match c {
            ' ' | '\t' | '\r' => {
                i += 1;
            }
            '#' => {
                while i < n && chars[i].1 != '\n' {
                    i += 1;
                }
            }
            '\n' => {
                let is_dup = matches!(toks.last().map(|t| &t.tok), Some(Tok::Newline) | None);
                if !is_dup {
                    toks.push(Token {
                        tok: Tok::Newline,
                        span: Span::new(pos, pos + 1),
                    });
                }
                i += 1;
            }
            '0'..='9' => {
                let start = pos;
                let mut j = i;
                let mut text = String::new();
                let mut is_float = false;
                while j < n {
                    let ch = chars[j].1;
                    if ch.is_ascii_digit() {
                        text.push(ch);
                        j += 1;
                    } else if ch == '_' && !text.is_empty() {
                        j += 1;
                    } else if ch == '.' && !is_float && j + 1 < n && chars[j + 1].1.is_ascii_digit()
                    {
                        is_float = true;
                        text.push('.');
                        j += 1;
                    } else if ch == '.' && j + 1 < n && chars[j + 1].1 == '.' {
                        // range operator follows; stop the number here
                        break;
                    } else {
                        break;
                    }
                }
                let stop = if j < n { chars[j].0 } else { end };
                let span = Span::new(start, stop);
                if is_float {
                    // f64 parse of [0-9_.]+ cannot fail for our restricted grammar
                    toks.push(Token {
                        tok: Tok::Float(text.parse::<f64>().unwrap_or(0.0)),
                        span,
                    });
                } else {
                    match text.parse::<i64>() {
                        Ok(v) => toks.push(Token {
                            tok: Tok::Int(v),
                            span,
                        }),
                        Err(_) => diags.push(
                            Diagnostic::error(
                                "E1101",
                                span,
                                format!("integer literal `{}` does not fit in 64 bits", text),
                            )
                            .with_hint("int is a signed 64-bit integer; the largest value is 9223372036854775807"),
                        ),
                    }
                }
                i = j;
            }
            '"' => {
                let start = pos;
                i += 1;
                let mut s = String::new();
                let mut closed = false;
                while i < n {
                    let (p, ch) = chars[i];
                    if ch == '"' {
                        i += 1;
                        closed = true;
                        break;
                    }
                    if ch == '\n' {
                        break;
                    }
                    if ch == '\\' {
                        i += 1;
                        if i >= n {
                            break;
                        }
                        let esc = chars[i].1;
                        match esc {
                            'n' => s.push('\n'),
                            't' => s.push('\t'),
                            'r' => s.push('\r'),
                            '0' => s.push('\0'),
                            '\\' => s.push('\\'),
                            '"' => s.push('"'),
                            other => diags.push(
                                Diagnostic::error(
                                    "E1001",
                                    Span::new(p, chars[i].0 + other.len_utf8()),
                                    format!("unknown escape sequence `\\{}`", other),
                                )
                                .with_hint("supported escapes: \\n \\t \\r \\0 \\\\ \\\""),
                            ),
                        }
                        i += 1;
                    } else {
                        s.push(ch);
                        i += 1;
                    }
                }
                if !closed {
                    diags.push(
                        Diagnostic::error(
                            "E1001",
                            Span::new(start, end.min(chars[i.saturating_sub(1)].0 + 1)),
                            "unterminated string literal",
                        )
                        .with_hint("add a closing `\"` on the same line"),
                    );
                } else {
                    toks.push(Token {
                        tok: Tok::Str(s),
                        span: Span::new(start, chars[i - 1].0 + 1),
                    });
                }
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = pos;
                let mut j = i;
                let mut text = String::new();
                while j < n {
                    let ch = chars[j].1;
                    if ch.is_alphanumeric() || ch == '_' {
                        text.push(ch);
                        j += 1;
                    } else {
                        break;
                    }
                }
                let stop = if j < n { chars[j].0 } else { end };
                let tok = keyword(&text).unwrap_or(Tok::Ident(text));
                toks.push(Token {
                    tok,
                    span: Span::new(start, stop),
                });
                i = j;
            }
            _ => {
                let next = if i + 1 < n {
                    Some(chars[i + 1].1)
                } else {
                    None
                };
                let (tok, width) = match (c, next) {
                    ('.', Some('.')) => {
                        if i + 2 < n && chars[i + 2].1 == '=' {
                            (Tok::DotDotEq, 3)
                        } else {
                            (Tok::DotDot, 2)
                        }
                    }
                    ('-', Some('>')) => (Tok::Arrow, 2),
                    ('=', Some('=')) => (Tok::EqEq, 2),
                    ('!', Some('=')) => (Tok::NotEq, 2),
                    ('<', Some('=')) => (Tok::Le, 2),
                    ('>', Some('=')) => (Tok::Ge, 2),
                    ('&', Some('&')) => (Tok::AmpAmp, 2),
                    ('|', Some('|')) => (Tok::PipePipe, 2),
                    ('+', Some('=')) => (Tok::PlusEq, 2),
                    ('-', Some('=')) => (Tok::MinusEq, 2),
                    ('*', Some('=')) => (Tok::StarEq, 2),
                    ('/', Some('=')) => (Tok::SlashEq, 2),
                    ('+', _) => (Tok::Plus, 1),
                    ('-', _) => (Tok::Minus, 1),
                    ('*', _) => (Tok::Star, 1),
                    ('/', _) => (Tok::Slash, 1),
                    ('%', _) => (Tok::Percent, 1),
                    ('=', _) => (Tok::Eq, 1),
                    ('<', _) => (Tok::Lt, 1),
                    ('>', _) => (Tok::Gt, 1),
                    ('!', _) => (Tok::Bang, 1),
                    ('(', _) => (Tok::LParen, 1),
                    (')', _) => (Tok::RParen, 1),
                    ('{', _) => (Tok::LBrace, 1),
                    ('}', _) => (Tok::RBrace, 1),
                    ('[', _) => (Tok::LBracket, 1),
                    (']', _) => (Tok::RBracket, 1),
                    (',', _) => (Tok::Comma, 1),
                    (':', _) => (Tok::Colon, 1),
                    (';', _) => (Tok::Semi, 1),
                    ('.', _) => (Tok::Dot, 1),
                    (other, _) => {
                        let w = other.len_utf8();
                        diags.push(
                            Diagnostic::error(
                                "E1001",
                                Span::new(pos, pos + w),
                                format!("unexpected character `{}`", other),
                            )
                            .with_hint("see the language reference for valid syntax"),
                        );
                        i += w;
                        continue;
                    }
                };
                toks.push(Token {
                    tok,
                    span: Span::new(pos, pos + width),
                });
                i += width;
            }
        }
    }

    toks.push(Token {
        tok: Tok::Eof,
        span: Span::new(end, end),
    });
    (toks, diags)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<Tok> {
        let (toks, diags) = lex(src);
        assert!(diags.is_empty(), "unexpected diagnostics: {:?}", diags);
        toks.into_iter().map(|t| t.tok).collect()
    }

    #[test]
    fn lexes_literals() {
        let ks = kinds("let x = 42\nlet y = 3.5");
        assert_eq!(
            ks,
            vec![
                Tok::KwLet,
                Tok::Ident("x".into()),
                Tok::Eq,
                Tok::Int(42),
                Tok::Newline,
                Tok::KwLet,
                Tok::Ident("y".into()),
                Tok::Eq,
                Tok::Float(3.5),
                Tok::Eof,
            ]
        );
    }

    #[test]
    fn lexes_strings_and_escapes() {
        let (toks, diags) = lex(r#"print("a\nb\\c")"#);
        assert!(diags.is_empty());
        assert_eq!(toks[2].tok, Tok::Str("a\nb\\c".into()));
    }

    #[test]
    fn lexes_operators() {
        let ks = kinds("== != <= >= && || + - * / % -> = += -=");
        assert!(ks.contains(&Tok::Arrow));
        assert!(ks.contains(&Tok::PlusEq));
        assert!(ks.contains(&Tok::AmpAmp));
    }

    #[test]
    fn collapses_blank_lines() {
        let ks = kinds("a\n\n\nb");
        assert_eq!(
            ks,
            vec![
                Tok::Ident("a".into()),
                Tok::Newline,
                Tok::Ident("b".into()),
                Tok::Eof
            ]
        );
    }

    #[test]
    fn reports_unterminated_string() {
        let (_, diags) = lex("let s = \"oops\n");
        assert_eq!(diags[0].code, "E1001");
    }

    #[test]
    fn reports_unknown_escape() {
        let (_, diags) = lex(r#""\q""#);
        assert_eq!(diags[0].code, "E1001");
    }

    #[test]
    fn reports_unexpected_char() {
        let (_, diags) = lex("let x = @");
        assert_eq!(diags[0].code, "E1001");
    }

    #[test]
    fn reports_integer_overflow() {
        let (_, diags) = lex("99999999999999999999999");
        assert_eq!(diags[0].code, "E1101");
    }

    #[test]
    fn comments_are_ignored() {
        let ks = kinds("# comment\nfn main() { } # trailing");
        assert!(ks.contains(&Tok::KwFn));
        assert!(!ks
            .iter()
            .any(|t| matches!(t, Tok::Ident(x) if x == "comment")));
    }

    #[test]
    fn underscore_separators() {
        assert_eq!(kinds("1_000")[0], Tok::Int(1000));
    }
}
