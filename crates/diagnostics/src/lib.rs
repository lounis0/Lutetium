//! Diagnostics infrastructure shared by every Lutetium crate.
//!
//! A `Diagnostic` carries a stable error code (`E####`), a severity, a message,
//! a source span and an optional hint. `SourceFile` renders diagnostics for
//! humans and machines (`diagnostics_json`).

use std::fmt::Write as _;

/// A half-open byte range `[lo, hi)` inside a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub lo: usize,
    pub hi: usize,
}

impl Span {
    pub fn new(lo: usize, hi: usize) -> Span {
        Span { lo, hi }
    }

    /// Smallest span covering both inputs.
    pub fn merge(self, other: Span) -> Span {
        Span {
            lo: self.lo.min(other.lo),
            hi: self.hi.max(other.hi),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    pub hint: Option<String>,
}

impl Diagnostic {
    pub fn error(code: &'static str, span: Span, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            code,
            severity: Severity::Error,
            message: message.into(),
            span,
            hint: None,
        }
    }

    pub fn warning(code: &'static str, span: Span, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            code,
            severity: Severity::Warning,
            message: message.into(),
            span,
            hint: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Diagnostic {
        self.hint = Some(hint.into());
        self
    }
}

/// A named source file with a line index for fast line/column lookup.
pub struct SourceFile {
    pub name: String,
    pub text: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    pub fn new(name: impl Into<String>, text: impl Into<String>) -> SourceFile {
        let text = text.into();
        let mut line_starts = vec![0usize];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        SourceFile {
            name: name.into(),
            text,
            line_starts,
        }
    }

    /// 1-based (line, column). Column counts chars, not bytes.
    pub fn line_col(&self, off: usize) -> (usize, usize) {
        let off = off.min(self.text.len());
        let line = match self.line_starts.binary_search(&off) {
            Ok(i) => i,
            Err(i) => i - 1,
        };
        let col = self.text[self.line_starts[line]..off].chars().count() + 1;
        (line + 1, col)
    }

    pub fn line_text(&self, line1: usize) -> &str {
        let idx = line1.saturating_sub(1);
        let start = self.line_starts[idx.min(self.line_starts.len() - 1)];
        let end = self
            .line_starts
            .get(idx + 1)
            .copied()
            .unwrap_or(self.text.len());
        self.text[start..end.min(self.text.len())].trim_end_matches(['\n', '\r'])
    }

    /// Human-readable multi-line diagnostic.
    pub fn render(&self, d: &Diagnostic) -> String {
        let (line, col) = self.line_col(d.span.lo);
        let mut s = String::new();
        let _ = writeln!(
            s,
            "{}:{}:{}: {}[{}]: {}",
            self.name,
            line,
            col,
            d.severity.as_str(),
            d.code,
            d.message
        );
        let src = self.line_text(line);
        let _ = writeln!(s, "  |");
        let _ = writeln!(s, "{:>4} | {}", line, src);
        // caret aligned under span
        let line_start = self.line_starts[line - 1];
        let prefix_chars = self.text[line_start..d.span.lo.min(self.text.len())]
            .chars()
            .count();
        let span_chars = self.text[d.span.lo.min(self.text.len())..d.span.hi.min(self.text.len())]
            .chars()
            .count();
        let _ = writeln!(
            s,
            "  | {}{}",
            " ".repeat(prefix_chars),
            "^".repeat(span_chars.max(1))
        );
        if let Some(h) = &d.hint {
            let _ = writeln!(s, "  = hint: {}", h);
        }
        s
    }
}

/// Escape a string for inclusion in JSON output.
pub fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

/// Machine-readable diagnostics for editor integration:
/// `lutetium check app.lu --diagnostics json`.
pub fn diagnostics_json(file: &SourceFile, diags: &[Diagnostic]) -> String {
    let mut out = String::new();
    out.push_str("{\"file\":\"");
    out.push_str(&json_escape(&file.name));
    out.push_str("\",\"diagnostics\":[");
    for (i, d) in diags.iter().enumerate() {
        let (line, col) = file.line_col(d.span.lo);
        let (end_line, end_col) = file.line_col(d.span.hi.max(d.span.lo));
        if i > 0 {
            out.push(',');
        }
        let _ = write!(
            out,
            "{{\"code\":\"{}\",\"severity\":\"{}\",\"line\":{},\"column\":{},\"endLine\":{},\"endColumn\":{},\"message\":\"{}\"",
            d.code,
            d.severity.as_str(),
            line,
            col,
            end_line,
            end_col,
            json_escape(&d.message)
        );
        if let Some(h) = &d.hint {
            let _ = write!(out, ",\"hint\":\"{}\"", json_escape(h));
        }
        out.push('}');
    }
    out.push_str("]}");
    out
}
