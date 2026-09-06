//! Pure fold-region detectors for brace-delimited, indentation-based, and
//! YAML-style languages, intended for consumption by language-provider plugins.
//!
//! Public functions share the `crate::fold::FoldRegion` output type:
//!
//! * `brace_fold` — character-level lexer-lite state machine that matches
//!   `{`/`}` pairs, skipping braces inside line/block comments, double-quoted
//!   strings (with `\"` escapes), Rust raw strings (`r"…"`, `r#"…"#`, …), and
//!   Go backtick strings.
//! * `brace_fold_with_brackets` — same state machine, additionally matching
//!   `[`/`]` pairs.  Used by the JSON plugin, where multiline arrays are as
//!   foldable as objects; kept separate from `brace_fold` so Rust/Go (where
//!   folding every multiline array literal would be noise) are unaffected.
//! * `shell_brace_fold` — shell-specific brace detector.  Matches `{`/`}`
//!   pairs, skipping braces inside `#` line comments, single-quoted strings
//!   (no escape processing), double-quoted strings (`\"` escapes), and
//!   heredocs (`<<WORD … WORD`).
//! * `hcl_brace_fold` — HCL-specific brace detector.  Matches `{`/`}`
//!   pairs, skipping braces inside `#`, `//`, and `/* … */` comments,
//!   double-quoted strings (`\"` escapes), and HCL heredocs
//!   (`<<EOF`, `<<'EOF'`, `<<"EOF"`, `<<-EOF`).
//! * `indent_fold` — Python-style indentation detector.  A region spans from
//!   each compound-statement header (`def`/`class`/`if`/`for`/`while`/etc.) to
//!   the last more-indented line.  Continuation keywords (`else`/`elif`/
//!   `except`/`finally`) are not new headers.  Blank lines are transparent.
//! * `yaml_fold` — YAML indentation detector. A region spans from any
//!   non-blank line to the last line more indented than it. This is the
//!   original built-in YAML fold algorithm; `crate::yaml_fold::detect_fold_regions`
//!   re-exports it so existing call sites are unaffected.
//! * `section_fold` — TOML table-section detector. A region spans from a
//!   `[table]` or `[[array]]` header to the line before the next header.
//! * `template_fold` — delimiter-aware template block detector for Go templates,
//!   Helm, and Jinja2.
//!
//! None of these functions know about `App`, plugins, or IPC — they are pure
//! transformations to `Vec<FoldRegion>`.

use crate::fold::FoldRegion;

// ---------------------------------------------------------------------------
// Brace-nesting detector
// ---------------------------------------------------------------------------

/// Detects foldable regions in brace-delimited languages (Rust, Go, C, Java,
/// JS, …).
///
/// Walks `text` character by character, maintaining a nesting stack for `{…}`
/// pairs.  Braces inside the following contexts are ignored:
///
/// * Line comments (`// …`)
/// * Block comments (`/* … */`)
/// * Double-quoted strings (`"…"` with `\"` escapes)
/// * Rust raw strings (`r"…"`, `r#"…"#`, `r##"…"##`, …)
/// * Go backtick strings (`` `…` ``)
///
/// Returns one region per `{…}` block that spans more than one line.  The
/// nesting stack is `Vec<usize>` (line number), so deeply nested files are
/// bounded only by available memory.
pub fn brace_fold(text: &str) -> Vec<FoldRegion> {
    brace_fold_impl(text, false)
}

/// Like [`brace_fold`], but also tracks `[…]` bracket blocks as foldable
/// regions — for JSON, a multiline array is as foldable as an object.
/// Braces and brackets share a single line-position stack (no type
/// checking), matching `brace_fold`'s existing tolerance of unbalanced
/// input: a `]` closes whatever is on top of the stack, `{` or `[` alike.
pub fn brace_fold_with_brackets(text: &str) -> Vec<FoldRegion> {
    brace_fold_impl(text, true)
}

fn brace_fold_impl(text: &str, track_brackets: bool) -> Vec<FoldRegion> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    if len == 0 {
        return Vec::new();
    }

    #[derive(Clone, Copy)]
    enum St {
        Normal,
        LineCmt,
        BlockCmt,
        DqStr,
        SqStr,
        RawStr(usize),
        BtStr,
    }

    let mut st = St::Normal;
    let mut line = 0usize;
    let mut stack: Vec<usize> = Vec::new();
    let mut regions: Vec<FoldRegion> = Vec::new();
    let mut i = 0;

    while i < len {
        let b = bytes[i];
        match st {
            St::Normal => match b {
                b'\n' => line += 1,
                b'{' => stack.push(line),
                b'}' => {
                    if let Some(start) = stack.pop() {
                        if line > start {
                            regions.push(FoldRegion { start, end: line });
                        }
                    }
                }
                b'[' if track_brackets => stack.push(line),
                b']' if track_brackets => {
                    if let Some(start) = stack.pop() {
                        if line > start {
                            regions.push(FoldRegion { start, end: line });
                        }
                    }
                }
                b'/' if i + 1 < len => match bytes[i + 1] {
                    b'/' => {
                        st = St::LineCmt;
                        i += 1;
                    }
                    b'*' => {
                        st = St::BlockCmt;
                        i += 1;
                    }
                    _ => {}
                },
                b'"' => st = St::DqStr,
                b'\'' => st = St::SqStr,
                b'`' => st = St::BtStr,
                b'r' => {
                    let prev_ident =
                        i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
                    if !prev_ident {
                        let mut j = i + 1;
                        let mut hashes = 0usize;
                        while j < len && bytes[j] == b'#' {
                            hashes += 1;
                            j += 1;
                        }
                        if j < len && bytes[j] == b'"' {
                            st = St::RawStr(hashes);
                            i = j;
                        }
                    }
                }
                _ => {}
            },
            St::LineCmt => {
                if b == b'\n' {
                    st = St::Normal;
                    line += 1;
                }
            }
            St::BlockCmt => {
                if b == b'\n' {
                    line += 1;
                } else if b == b'*' && i + 1 < len && bytes[i + 1] == b'/' {
                    st = St::Normal;
                    i += 1;
                }
            }
            St::DqStr => {
                if b == b'\\' && i + 1 < len {
                    if bytes[i + 1] == b'\n' {
                        line += 1;
                    }
                    i += 1;
                } else if b == b'"' {
                    st = St::Normal;
                } else if b == b'\n' {
                    line += 1;
                }
            }
            St::SqStr => {
                if b == b'\\' {
                    i += 1;
                } else if b == b'\'' {
                    st = St::Normal;
                }
            }
            St::RawStr(hashes) => {
                if b == b'\n' {
                    line += 1;
                } else if b == b'"' {
                    let mut j = i + 1;
                    let mut seen = 0usize;
                    while j < len && bytes[j] == b'#' && seen < hashes {
                        seen += 1;
                        j += 1;
                    }
                    if seen == hashes {
                        st = St::Normal;
                        i = j.wrapping_sub(1);
                    }
                }
            }
            St::BtStr => {
                if b == b'`' {
                    st = St::Normal;
                } else if b == b'\n' {
                    line += 1;
                }
            }
        }
        i += 1;
    }

    regions
}

// ---------------------------------------------------------------------------
// Shell brace-nesting detector
// ---------------------------------------------------------------------------

/// Detects foldable regions in shell scripts (sh, bash, zsh).
///
/// Like [`brace_fold`], walks `text` character by character maintaining a
/// nesting stack for `{…}` pairs, but uses shell-specific syntax rules:
///
/// * Line comments (`# …` at word start only — `$#`/`${#v}` are expansions)
/// * Single-quoted strings (`'…'` — no escape processing)
/// * Double-quoted strings (`"…"` with `\"` escapes)
/// * Backslash escapes outside strings (`\'` does not open a string)
/// * Heredocs (`<<WORD`, `<<'WORD'`, `<<-WORD` … `WORD` — braces inside are inert)
/// * Arithmetic contexts (`(( … ))` — `<<` inside is a shift, not a heredoc)
///
/// Returns one region per `{…}` block that spans more than one line.
pub fn shell_brace_fold(text: &str) -> Vec<FoldRegion> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    if len == 0 {
        return Vec::new();
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum St {
        Normal,
        LineCmt,
        SqStr,
        DqStr,
        Heredoc,
    }

    let mut st = St::Normal;
    let mut line = 0usize;
    let mut stack: Vec<usize> = Vec::new();
    let mut regions: Vec<FoldRegion> = Vec::new();
    let mut heredoc_delim: Vec<u8> = Vec::new();
    // Only <<- heredocs allow (tab) indentation before the closing delimiter.
    let mut heredoc_allow_indent = false;
    // Depth of `(( … ))` arithmetic contexts, where << is a shift operator.
    let mut arith_depth = 0usize;
    let mut i = 0;

    while i < len {
        let b = bytes[i];
        match st {
            St::Normal => match b {
                b'\n' => line += 1,
                b'\\' if i + 1 < len => {
                    // Escape outside strings: \' and \" must not open a string.
                    if bytes[i + 1] == b'\n' {
                        line += 1;
                    }
                    i += 1;
                }
                b'{' => stack.push(line),
                b'}' => {
                    if let Some(start) = stack.pop() {
                        if line > start {
                            regions.push(FoldRegion { start, end: line });
                        }
                    }
                }
                // `#` starts a comment only at the start of a word; `$#` and
                // `${#var}` are parameter expansions.
                b'#' if i == 0
                    || matches!(
                        bytes[i - 1],
                        b'\n' | b' ' | b'\t' | b';' | b'&' | b'|' | b'('
                    ) =>
                {
                    st = St::LineCmt
                }
                b'\'' => st = St::SqStr,
                b'"' => st = St::DqStr,
                b'(' if i + 1 < len && bytes[i + 1] == b'(' => {
                    arith_depth += 1;
                    i += 1;
                }
                b')' if arith_depth > 0 && i + 1 < len && bytes[i + 1] == b')' => {
                    arith_depth -= 1;
                    i += 1;
                }
                b'<' if i + 1 < len && bytes[i + 1] == b'<' => {
                    if arith_depth > 0 {
                        // Shift operator inside (( … )) — not a heredoc.
                        i += 1;
                    } else if i + 2 < len && bytes[i + 2] == b'<' {
                        // Skip <<< (here-string) — not a heredoc.
                        i += 2;
                    } else {
                        i += 1;
                        let dash = i + 1 < len && bytes[i + 1] == b'-';
                        if dash {
                            i += 1;
                        }
                        i += 1;
                        while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                            i += 1;
                        }
                        let word_start = i;
                        while i < len && bytes[i] != b'\n' && bytes[i] != b' ' && bytes[i] != b'\t'
                        {
                            i += 1;
                        }
                        // Strip quoting from the delimiter word: <<'EOF',
                        // <<"EOF" and <<\EOF all terminate on a bare EOF line.
                        let mut word = &bytes[word_start..i];
                        if word.first() == Some(&b'\\') {
                            word = &word[1..];
                        } else if word.len() >= 2
                            && (word[0] == b'\'' || word[0] == b'"')
                            && word[word.len() - 1] == word[0]
                        {
                            word = &word[1..word.len() - 1];
                        }
                        if !word.is_empty() {
                            heredoc_delim.clear();
                            heredoc_delim.extend_from_slice(word);
                            heredoc_allow_indent = dash;
                            st = St::Heredoc;
                            continue;
                        }
                    }
                }
                _ => {}
            },
            St::LineCmt => {
                if b == b'\n' {
                    st = St::Normal;
                    line += 1;
                }
            }
            St::SqStr => {
                if b == b'\'' {
                    st = St::Normal;
                } else if b == b'\n' {
                    line += 1;
                }
            }
            St::DqStr => {
                if b == b'\\' && i + 1 < len {
                    if bytes[i + 1] == b'\n' {
                        line += 1;
                    }
                    i += 1;
                } else if b == b'"' {
                    st = St::Normal;
                } else if b == b'\n' {
                    line += 1;
                }
            }
            St::Heredoc => {
                if b == b'\n' {
                    line += 1;
                    let after_nl = i + 1;
                    let remaining = len - after_nl;
                    if remaining >= heredoc_delim.len() {
                        // Only <<- strips leading tabs before the delimiter;
                        // a plain << delimiter must start the line.
                        let mut start = after_nl;
                        if heredoc_allow_indent {
                            while start < len && bytes[start] == b'\t' {
                                start += 1;
                            }
                        }
                        let avail = len - start;
                        if avail >= heredoc_delim.len() {
                            let mut matches = true;
                            for (j, &d) in heredoc_delim.iter().enumerate() {
                                if bytes[start + j] != d {
                                    matches = false;
                                    break;
                                }
                            }
                            if matches {
                                let after_delim = start + heredoc_delim.len();
                                if after_delim >= len
                                    || bytes[after_delim] == b'\n'
                                    || bytes[after_delim] == b'\r'
                                {
                                    st = St::Normal;
                                }
                            }
                        }
                    }
                }
            }
        }
        i += 1;
    }

    regions
}

// ---------------------------------------------------------------------------
// HCL brace-nesting detector
// ---------------------------------------------------------------------------

/// Detects foldable regions in HashiCorp Configuration Language (HCL) files —
/// Terraform, TFLint, OpenBao, Nomad, Consul.
///
/// Like [`brace_fold`], walks `text` character by character maintaining a
/// nesting stack for `{…}` pairs, but uses HCL-specific syntax rules:
///
/// * Line comments (`# …` and `// …`)
/// * Block comments (`/* … */`)
/// * Double-quoted strings (`"…"` with `\"` escapes)
/// * Heredocs (`<<WORD`, `<<'WORD'`, `<<"WORD"`, `<<-WORD` … `WORD` — braces
///   inside are inert)
///
/// Returns one region per `{…}` block that spans more than one line.
pub fn hcl_brace_fold(text: &str) -> Vec<FoldRegion> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    if len == 0 {
        return Vec::new();
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum St {
        Normal,
        LineCmt,
        BlockCmt,
        DqStr,
        Heredoc,
    }

    let mut st = St::Normal;
    let mut line = 0usize;
    let mut stack: Vec<usize> = Vec::new();
    let mut regions: Vec<FoldRegion> = Vec::new();
    let mut heredoc_delim: Vec<u8> = Vec::new();
    // Only <<- heredocs allow (tab) indentation before the closing delimiter.
    let mut heredoc_allow_indent = false;
    let mut i = 0;

    while i < len {
        let b = bytes[i];
        match st {
            St::Normal => match b {
                b'\n' => line += 1,
                b'{' => stack.push(line),
                b'}' => {
                    if let Some(start) = stack.pop() {
                        if line > start {
                            regions.push(FoldRegion { start, end: line });
                        }
                    }
                }
                // `#` and `//` start a comment that runs to end of line.
                b'#' => st = St::LineCmt,
                b'/' if i + 1 < len && bytes[i + 1] == b'/' => {
                    st = St::LineCmt;
                    i += 1;
                }
                b'/' if i + 1 < len && bytes[i + 1] == b'*' => {
                    st = St::BlockCmt;
                    i += 1;
                }
                b'"' => st = St::DqStr,
                b'<' if i + 1 < len && bytes[i + 1] == b'<' => {
                    i += 1;
                    let dash = i + 1 < len && bytes[i + 1] == b'-';
                    if dash {
                        i += 1;
                    }
                    i += 1;
                    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                        i += 1;
                    }
                    let word_start = i;
                    while i < len && bytes[i] != b'\n' && bytes[i] != b' ' && bytes[i] != b'\t' {
                        i += 1;
                    }
                    // Strip quoting from the delimiter word: <<'EOF' and
                    // <<"EOF" both terminate on a bare EOF line.
                    let mut word = &bytes[word_start..i];
                    if word.len() >= 2
                        && (word[0] == b'\'' || word[0] == b'"')
                        && word[word.len() - 1] == word[0]
                    {
                        word = &word[1..word.len() - 1];
                    }
                    if !word.is_empty() {
                        heredoc_delim.clear();
                        heredoc_delim.extend_from_slice(word);
                        heredoc_allow_indent = dash;
                        st = St::Heredoc;
                        continue;
                    }
                }
                _ => {}
            },
            St::LineCmt => {
                if b == b'\n' {
                    st = St::Normal;
                    line += 1;
                }
            }
            St::BlockCmt => {
                if b == b'\n' {
                    line += 1;
                } else if b == b'*' && i + 1 < len && bytes[i + 1] == b'/' {
                    st = St::Normal;
                    i += 1;
                }
            }
            St::DqStr => {
                if b == b'\\' && i + 1 < len {
                    if bytes[i + 1] == b'\n' {
                        line += 1;
                    }
                    i += 1;
                } else if b == b'"' {
                    st = St::Normal;
                } else if b == b'\n' {
                    line += 1;
                }
            }
            St::Heredoc => {
                if b == b'\n' {
                    line += 1;
                    let after_nl = i + 1;
                    let remaining = len - after_nl;
                    if remaining >= heredoc_delim.len() {
                        // Only <<- strips leading whitespace before the
                        // delimiter; a plain << delimiter must start the line.
                        let mut start = after_nl;
                        if heredoc_allow_indent {
                            while start < len && (bytes[start] == b'\t' || bytes[start] == b' ') {
                                start += 1;
                            }
                        }
                        let avail = len - start;
                        if avail >= heredoc_delim.len() {
                            let mut matches = true;
                            for (j, &d) in heredoc_delim.iter().enumerate() {
                                if bytes[start + j] != d {
                                    matches = false;
                                    break;
                                }
                            }
                            if matches {
                                let after_delim = start + heredoc_delim.len();
                                if after_delim >= len
                                    || bytes[after_delim] == b'\n'
                                    || bytes[after_delim] == b'\r'
                                {
                                    st = St::Normal;
                                }
                            }
                        }
                    }
                }
            }
        }
        i += 1;
    }

    regions
}

// ---------------------------------------------------------------------------
// Indentation-based detector (Python)
// ---------------------------------------------------------------------------

const PY_CONTINUATIONS: &[&str] = &["else", "elif", "except", "finally"];

const PY_HEADERS: &[&str] = &[
    "def", "class", "if", "for", "while", "with", "try", "match", "case",
];

/// Returns `true` when `line` is a comment (`#…`) after stripping leading
/// whitespace. Comments carry no indentation significance in Python, so they
/// must not terminate a fold region even when dedented to (or past) the
/// enclosing header's indent.
fn is_comment_line(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

/// Returns `true` when `line` starts with a continuation keyword (`else`,
/// `elif`, `except`, `finally`) after stripping leading whitespace.
fn is_py_continuation(line: &str) -> bool {
    let trimmed = line.trim_start();
    let word_end = trimmed
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(trimmed.len());
    let first = &trimmed[..word_end];
    PY_CONTINUATIONS.contains(&first)
}

/// Returns `true` when `line` starts with a compound-statement header keyword.
///
/// Recognised: `def`, `class`, `if`, `for`, `while`, `with`, `try`, `match`,
/// `case`, and `@decorator` lines.  The `async` prefix is handled so that
/// `async def`/`async for`/`async with` are also headers.
fn is_py_header(line: &str) -> bool {
    let trimmed = line.trim_start();
    let word_end = trimmed
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(trimmed.len());
    let first = &trimmed[..word_end];

    // `async def`/`async for`/`async with` — treat as headers.
    if first == "async" {
        let rest = trimmed[word_end..].trim_start();
        let next_end = rest
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(rest.len());
        let second = &rest[..next_end];
        return matches!(second, "def" | "for" | "with");
    }

    PY_HEADERS.contains(&first)
}

/// Detects foldable regions in Python-style indentation-based files.
///
/// A region runs from each compound-statement header (see `is_py_header`) to
/// the last line that is more indented.  Continuation keywords (`else`,
/// `elif`, `except`, `finally`) do **not** start new regions — they are
/// considered part of the preceding statement.  Blank lines are transparent
/// and do not terminate a region.
pub fn indent_fold(text: &str) -> Vec<FoldRegion> {
    let lines: Vec<&str> = text.lines().collect();
    let n = lines.len();
    if n == 0 {
        return Vec::new();
    }

    // Per-line leading whitespace count.  Blank lines → None.
    let indent: Vec<Option<usize>> = lines
        .iter()
        .map(|l| {
            let trimmed = l.trim_start();
            if trimmed.is_empty() {
                None
            } else {
                Some(l.len() - trimmed.len())
            }
        })
        .collect();

    let mut regions = Vec::new();

    for i in 0..n {
        let Some(curr_indent) = indent[i] else {
            continue;
        };

        // Only compound-statement headers initiate fold regions.
        if !is_py_header(lines[i]) {
            continue;
        }

        // Walk forward: the region extends through every line that is blank,
        // a continuation at the header's level, or strictly more-indented.
        let mut end = i;
        let mut j = i + 1;
        while j < n {
            match indent[j] {
                None => {
                    // Blank line — does not terminate the region.
                    end = j;
                    j += 1;
                }
                Some(ind) if ind > curr_indent => {
                    // More deeply indented → still inside the block.
                    end = j;
                    j += 1;
                }
                Some(_) if is_py_continuation(lines[j]) => {
                    // Continuation at same/lesser indent — pass through.
                    end = j;
                    j += 1;
                }
                Some(_) if is_comment_line(lines[j]) => {
                    // Comments have no indentation significance — pass through
                    // regardless of their column.
                    end = j;
                    j += 1;
                }
                Some(_) => {
                    // Non-blank, non-continuation at same or lesser indent
                    // terminates the block.
                    break;
                }
            }
        }

        if end > i {
            regions.push(FoldRegion { start: i, end });
        }
    }

    regions
}

/// Detects foldable TOML table sections.
///
/// A section starts at a valid table header (`[table]`) or array-of-tables
/// header (`[[table]]`) and extends through the line before the next header.
/// Headers with trailing comments are accepted, while bracket-like text in
/// values or comments is ignored because only the trimmed start of a line is
/// considered. Single-line sections are omitted.
pub fn section_fold(text: &str) -> Vec<FoldRegion> {
    let lines: Vec<&str> = text.lines().collect();
    let headers: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter_map(|(line, text)| is_toml_header(text).then_some(line))
        .collect();

    headers
        .iter()
        .enumerate()
        .filter_map(|(index, &start)| {
            let end = headers
                .get(index + 1)
                .copied()
                .unwrap_or(lines.len())
                .saturating_sub(1);
            (end > start).then_some(FoldRegion { start, end })
        })
        .collect()
}

/// Detects structural blocks in Go-template and Jinja2 template files.
///
/// Matching directives are `define`/`if`/`range`/`with` and `block`/`if`/
/// `for`/`macro`, respectively. Template comments are removed from the scan,
/// including multiline comments, so directive-looking text inside comments
/// cannot create or close a region. Regions are returned in closing order,
/// matching the existing brace detector convention.
pub fn template_fold(text: &str) -> Vec<FoldRegion> {
    let mut stack: Vec<(String, usize)> = Vec::new();
    let mut regions = Vec::new();
    let mut i = 0;
    let mut line = 0;
    let bytes = text.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'\n' {
            line += 1;
            i += 1;
            continue;
        }
        if text.get(i..).is_some_and(|rest| rest.starts_with("{{/*")) {
            if let Some(end) = text[i + 4..].find("*/}}") {
                line += text[i + 4..i + 4 + end]
                    .bytes()
                    .filter(|&b| b == b'\n')
                    .count();
                i += end + 8;
                continue;
            }
            break;
        }
        let (open, close) = if text.get(i..).is_some_and(|rest| rest.starts_with("{{")) {
            ("{{", "}}")
        } else if text.get(i..).is_some_and(|rest| rest.starts_with("{%")) {
            ("{%", "%}")
        } else if text.get(i..).is_some_and(|rest| rest.starts_with("{#")) {
            if let Some(end) = text[i + 2..].find("#}") {
                line += text[i + 2..i + 2 + end]
                    .bytes()
                    .filter(|&b| b == b'\n')
                    .count();
                i += end + 4;
                continue;
            }
            break;
        } else {
            i += 1;
            continue;
        };
        let Some(end) = text[i + 2..].find(close) else {
            break;
        };
        let content = text[i + 2..i + 2 + end].trim_matches('-').trim();
        let first = content.split_whitespace().next().unwrap_or("");
        if content.starts_with("/*") {
            line += content.bytes().filter(|&b| b == b'\n').count();
        } else if is_template_open(first, open) {
            stack.push((first.to_string(), line));
        } else if is_template_close(first) {
            if let Some((_, start)) = stack.pop() {
                if line > start {
                    regions.push(FoldRegion { start, end: line });
                }
            }
        }
        let consumed = &text[i..i + 2 + end + 2];
        line += consumed.bytes().filter(|&b| b == b'\n').count();
        i += 2 + end + 2;
    }
    regions
}

fn is_template_open(word: &str, delimiter: &str) -> bool {
    (delimiter == "{{" && matches!(word, "define" | "if" | "range" | "with"))
        || (delimiter == "{%" && matches!(word, "block" | "if" | "for" | "macro"))
}

fn is_template_close(word: &str) -> bool {
    matches!(word, "end" | "endblock" | "endif" | "endfor" | "endmacro")
}

fn is_toml_header(line: &str) -> bool {
    let trimmed = line
        .split_once('#')
        .map_or(line, |(before, _)| before)
        .trim();
    let Some(rest) = trimmed.strip_prefix('[') else {
        return false;
    };
    let rest = rest.strip_prefix('[').unwrap_or(rest);
    let closing = if trimmed.starts_with("[[") { "]]" } else { "]" };
    let Some(header) = rest.strip_suffix(closing) else {
        return false;
    };
    let header = header
        .split_once('#')
        .map_or(header, |(name, _)| name)
        .trim();
    !header.is_empty() && !header.contains('[') && !header.contains(']')
}

// ---------------------------------------------------------------------------
// YAML indentation-based detector
// ---------------------------------------------------------------------------

/// Detects foldable regions in YAML content by indentation nesting.
///
/// Delegates to `crate::yaml_fold::detect_fold_regions` — the original
/// built-in YAML fold algorithm — so this crate has a single implementation
/// that both the app's built-in YAML handling and the bundled `yaml`
/// language-provider plugin (`plugins/yaml`) share.
pub fn yaml_fold(lines: &[impl AsRef<str>]) -> Vec<FoldRegion> {
    crate::yaml_fold::detect_fold_regions(lines)
}

#[cfg(test)]
#[path = "fold_detectors_test.rs"]
mod tests;
