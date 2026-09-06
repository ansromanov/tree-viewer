use super::*;

// ---------------------------------------------------------------------------
// brace_fold tests
// ---------------------------------------------------------------------------

#[test]
fn brace_fold_empty() {
    assert!(brace_fold("").is_empty());
}

#[test]
fn brace_fold_no_braces() {
    assert!(brace_fold("fn foo() -> i32 { 0 }").is_empty());
}

#[test]
fn brace_fold_single_line_block() {
    // Single-line {…} — no region (must span >1 line).
    assert!(brace_fold("fn foo() -> i32 { 42 }").is_empty());
}

#[test]
fn brace_fold_simple_block() {
    let r = brace_fold("fn foo() {\n    bar();\n}\n");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 2);
}

#[test]
fn brace_fold_nested_blocks() {
    let r = brace_fold(
        "\
fn outer() {
    fn inner() {
        x();
    }
    y();
}
",
    );
    assert_eq!(r.len(), 2);
    // Regions are returned in closing order: inner closes first.
    assert_eq!(r[0].start, 1);
    assert_eq!(r[0].end, 3);
    // Outer block covers lines 0–5 (inclusive).
    assert_eq!(r[1].start, 0);
    assert_eq!(r[1].end, 5);
}

#[test]
fn brace_fold_skips_line_comment() {
    // Braces inside // comments should be skipped.
    let r = brace_fold(
        "\
fn foo() {
    // { this brace is ignored }
    bar();
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 3);
}

#[test]
fn brace_fold_skips_block_comment() {
    let r = brace_fold(
        "\
fn foo() {
    /* { block brace } */
    bar();
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 3);
}

#[test]
fn brace_fold_skips_block_comment_multiline() {
    let r = brace_fold(
        "\
fn foo() {
    /*
       { nested brace in block comment }
    */
    bar();
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 5);
}

#[test]
fn brace_fold_skips_double_quoted_string() {
    let r = brace_fold(
        "\
fn foo() {
    let s = \"hello { world }\";
    bar();
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 3);
}

#[test]
fn brace_fold_skips_escaped_quote() {
    let r = brace_fold(
        "\
fn foo() {
    let s = \"escaped \\\" quote { brace }\";
    bar();
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 3);
}

#[test]
fn brace_fold_escaped_newline_in_string_keeps_line_count() {
    // A backslash immediately followed by a real newline (C/JS line
    // continuation inside a string) must still advance the line counter.
    let r = brace_fold("int foo() {\n    char *s = \"abc\\\ndef\";\n    bar();\n}\n");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 4);
}

#[test]
fn brace_fold_skips_raw_string() {
    let r = brace_fold(
        "\
fn foo() {
    let s = r\"hello { world }\";
    bar();
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 3);
}

#[test]
fn brace_fold_skips_raw_string_with_hashes() {
    let r = brace_fold(
        "\
fn foo() {
    let s = r#\"hello { \"quoted\" } \"#;
    bar();
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 3);
}

#[test]
fn brace_fold_skips_raw_string_many_hashes() {
    let r = brace_fold(
        "\
fn foo() {
    let s = r##\"hello # { world } \"##;
    bar();
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 3);
}

#[test]
fn brace_fold_skips_backtick_string() {
    let r = brace_fold(
        "\
func foo() {
    s := `hello { world }`
    bar()
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 3);
}

#[test]
fn brace_fold_skips_single_quoted_string() {
    let r = brace_fold(".icon {\n  content: '}' ;\n}\n");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 2);
}

#[test]
fn section_fold_handles_tables_and_comments() {
    let r = section_fold(
        "title = \"demo\"\n[package] # metadata\nname = \"x\"\n[[bin]]\nname = \"x\"\n",
    );
    assert_eq!(
        r.iter().map(|x| (x.start, x.end)).collect::<Vec<_>>(),
        vec![(1, 2), (3, 4)]
    );
}

#[test]
fn template_fold_handles_nested_blocks_and_comments() {
    let r = template_fold(
        "{{ define \"x\" }}\n{{ if .Enabled }}\n{{/* {{ end }} */}}\nvalue\n{{ end }}\n{{ end }}\n",
    );
    assert_eq!(
        r.iter().map(|x| (x.start, x.end)).collect::<Vec<_>>(),
        vec![(1, 4), (0, 5)]
    );
}

#[test]
fn template_fold_handles_go_block_directive() {
    let regions = template_fold("{{ block \"title\" . }}\ncontent\n{{ end }}\n");
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].start, 0);
    assert_eq!(regions[0].end, 2);
}

#[test]
fn brace_fold_backtick_newline() {
    let r = brace_fold(
        "\
func foo() {
    s := `hello
{ world }`
    bar()
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 4);
}

#[test]
fn brace_fold_crlf() {
    let r = brace_fold("fn foo() {\r\n    bar();\r\n}\r\n");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 2);
}

#[test]
fn brace_fold_deep_nesting() {
    let r = brace_fold(
        "\
a {
    b {
        c {
            d { e() }
        }
    }
}
",
    );
    assert_eq!(r.len(), 3);
    // Regions are returned in closing (innermost-first) order.
    // c { … }
    assert_eq!(r[0].start, 2);
    assert_eq!(r[0].end, 4);
    // b { … }
    assert_eq!(r[1].start, 1);
    assert_eq!(r[1].end, 5);
    // a { … }
    assert_eq!(r[2].start, 0);
    assert_eq!(r[2].end, 6);
}

#[test]
fn brace_fold_unmatched_close_is_silent() {
    // Extra } with no matching { should be silently ignored.
    let r = brace_fold("}\n{\n    foo();\n}\n");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 1);
    assert_eq!(r[0].end, 3);
}

#[test]
fn brace_fold_unmatched_open_leaves_stack() {
    // Extra { with no matching } leaves it on the stack but doesn't panic.
    let r = brace_fold("{\n    foo();\n");
    assert!(r.is_empty());
}

#[test]
fn brace_fold_ignores_brackets() {
    // Plain brace_fold must not fold `[...]` blocks — brackets are inert.
    let r = brace_fold("[\n    1,\n    2\n]\n");
    assert!(r.is_empty());
}

// ---------------------------------------------------------------------------
// brace_fold_with_brackets tests
// ---------------------------------------------------------------------------

#[test]
fn brace_fold_with_brackets_empty() {
    assert!(brace_fold_with_brackets("").is_empty());
}

#[test]
fn brace_fold_with_brackets_object_block() {
    let r = brace_fold_with_brackets("{\n    \"a\": 1\n}\n");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 2);
}

#[test]
fn brace_fold_with_brackets_array_block() {
    let r = brace_fold_with_brackets("[\n    1,\n    2\n]\n");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 3);
}

#[test]
fn brace_fold_with_brackets_single_line_array_no_region() {
    let r = brace_fold_with_brackets("[1, 2, 3]");
    assert!(r.is_empty());
}

#[test]
fn brace_fold_with_brackets_nested_object_and_array() {
    let r = brace_fold_with_brackets(
        "\
{
    \"items\": [
        1,
        2
    ]
}
",
    );
    assert_eq!(r.len(), 2);
    // Inner array closes first: lines 1-4.
    assert_eq!(r[0].start, 1);
    assert_eq!(r[0].end, 4);
    // Outer object: lines 0-5.
    assert_eq!(r[1].start, 0);
    assert_eq!(r[1].end, 5);
}

#[test]
fn brace_fold_with_brackets_bracket_in_string_ignored() {
    let r = brace_fold_with_brackets(
        "\
{
    \"note\": \"array looks like [ this ] but is just text\",
    \"n\": 1
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 3);
}

#[test]
fn brace_fold_with_brackets_mismatched_pair_silent() {
    // A `]` closing a `{` (or vice versa) should not panic or produce a
    // region — treat the stack purely as a line-position stack, matching
    // `brace_fold`'s existing tolerance of unbalanced input.
    let r = brace_fold_with_brackets("{\n    1\n]\n");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 2);
}

// ---------------------------------------------------------------------------
// indent_fold tests
// ---------------------------------------------------------------------------

#[test]
fn indent_fold_empty() {
    assert!(indent_fold("").is_empty());
}

#[test]
fn indent_fold_flat_code_no_regions() {
    let code = "\
x = 1
y = 2
z = 3
";
    assert!(indent_fold(code).is_empty());
}

#[test]
fn indent_fold_simple_def() {
    let r = indent_fold(
        "\
def foo():
    pass
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 1);
}

#[test]
fn indent_fold_nested_def() {
    let r = indent_fold(
        "\
class Outer:
    def inner(self):
        pass
    x = 1
",
    );
    assert_eq!(r.len(), 2);
    // Outer class: lines 0–3
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 3);
    // inner method: lines 1–2
    assert_eq!(r[1].start, 1);
    assert_eq!(r[1].end, 2);
}

#[test]
fn indent_fold_if_elif_else_continuation() {
    let r = indent_fold(
        "\
if True:
    x = 1
elif other:
    y = 2
else:
    z = 3
",
    );
    // Only one region: from `if` to end of `else` block.
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 5);
}

#[test]
fn indent_fold_try_except_finally() {
    let r = indent_fold(
        "\
try:
    do_something()
except ValueError:
    handle()
finally:
    cleanup()
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 5);
}

#[test]
fn indent_fold_blank_lines_dont_terminate() {
    let r = indent_fold(
        "\
def foo():
    x = 1

    y = 2

z = 3
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 4);
}

#[test]
fn indent_fold_async_def_header() {
    let r = indent_fold(
        "\
async def fetch():
    await something()
    return x
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 2);
}

#[test]
fn indent_fold_async_for_header() {
    let r = indent_fold(
        "\
async def gen():
    async for item in stream():
        process(item)
    return x
",
    );
    assert_eq!(r.len(), 2);
    // async def gen: lines 0–3
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 3);
    // async for item: lines 1–2
    assert_eq!(r[1].start, 1);
    assert_eq!(r[1].end, 2);
}

#[test]
fn indent_fold_with_decorator() {
    // Decorators are not headers — `def`/`class` below them is.
    let r = indent_fold(
        "\
@decorator
def foo():
    pass

@register
class Handler:
    def run(self):
        pass
",
    );
    assert_eq!(r.len(), 3);
    // def foo:  (decorator on line 0 is part of the same declaration, not a separate header)
    // Blank line after pass keeps the region going.
    assert_eq!(r[0].start, 1);
    assert_eq!(r[0].end, 3);
    // class Handler:
    assert_eq!(r[1].start, 5);
    assert_eq!(r[1].end, 7);
    // def run:
    assert_eq!(r[2].start, 6);
    assert_eq!(r[2].end, 7);
}

#[test]
fn indent_fold_crlf() {
    let r = indent_fold("def foo():\r\n    pass\r\n");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 1);
}

#[test]
fn indent_fold_continuation_not_pseudo_header() {
    // Lines starting with `elsewhere` or `exceptional` should NOT be treated
    // as continuations — only exact keyword match.
    let r = indent_fold(
        "\
if True:
    pass
elsewhere = 5
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 1);
}

#[test]
fn indent_fold_inner_else_continuation() {
    let r = indent_fold(
        "\
if a:
    if b:
        c()
    else:
        d()
e = 1
",
    );
    assert_eq!(r.len(), 2);
    // Outer if a: lines 0–4 (covers inner if/else including d())
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 4);
    // Inner if b: lines 1–4 (else continuation extends through d())
    assert_eq!(r[1].start, 1);
    assert_eq!(r[1].end, 4);
}

#[test]
fn indent_fold_match_case_header() {
    let r = indent_fold(
        "\
def process(val):
    match val:
        case 1:
            return 'one'
        case _:
            return 'other'
",
    );
    // def process, match, and each case arm create fold regions.
    assert_eq!(r.len(), 4);
    assert_eq!(r[0].start, 0); // def process: lines 0–5
    assert_eq!(r[0].end, 5);
    assert_eq!(r[1].start, 1); // match val: lines 1–5
    assert_eq!(r[1].end, 5);
    assert_eq!(r[2].start, 2); // case 1: lines 2–3
    assert_eq!(r[2].end, 3);
    assert_eq!(r[3].start, 4); // case _: lines 4–5
    assert_eq!(r[3].end, 5);
}

#[test]
fn indent_fold_multiple_top_level_headers() {
    let text = "\
def first():
    a()
    b()

def second():
    c()
    d()
";
    let r = indent_fold(text);
    assert_eq!(r.len(), 2);
    // Blank line after b() is included in the region.
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 3);
    assert_eq!(r[1].start, 4);
    assert_eq!(r[1].end, 6);
}

#[test]
fn indent_fold_while_for_headers() {
    let r = indent_fold(
        "\
while cond:
    process()
    if flag:
        break
",
    );
    assert_eq!(r.len(), 2);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 3);
    assert_eq!(r[1].start, 2);
    assert_eq!(r[1].end, 3);
}

#[test]
fn indent_fold_with_header() {
    let r = indent_fold(
        "\
with open('f') as f:
    data = f.read()
    process(data)
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 2);
}

#[test]
fn indent_fold_comment_lines_not_headers() {
    // Lines starting with # are blank for our purposes (trimmed empty).
    let r = indent_fold(
        "\
def foo():
    # comment
    pass
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 2);
}

#[test]
fn indent_fold_dedented_comment_does_not_terminate_region() {
    // A comment at column 0 (or any indent <= the header's) carries no
    // indentation significance in Python and must not end the block.
    let r = indent_fold(
        "\
def foo():
    x = 1
# section marker
    y = 2
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 3);
}

// ---------------------------------------------------------------------------
// shell_brace_fold tests
// ---------------------------------------------------------------------------

#[test]
fn shell_brace_fold_empty() {
    assert!(shell_brace_fold("").is_empty());
}

#[test]
fn shell_brace_fold_no_braces() {
    assert!(shell_brace_fold("echo hello\n").is_empty());
}

#[test]
fn shell_brace_fold_single_line_block() {
    assert!(shell_brace_fold("foo() { echo hi; }\n").is_empty());
}

#[test]
fn shell_brace_fold_simple_function() {
    let r = shell_brace_fold("foo() {\n    echo hi\n}\n");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 2);
}

#[test]
fn shell_brace_fold_nested_blocks() {
    let r = shell_brace_fold(
        "\
outer() {
    inner() {
        echo hi
    }
    echo done
}
",
    );
    assert_eq!(r.len(), 2);
    assert_eq!(r[0].start, 1);
    assert_eq!(r[0].end, 3);
    assert_eq!(r[1].start, 0);
    assert_eq!(r[1].end, 5);
}

#[test]
fn shell_brace_fold_skips_line_comment() {
    let r = shell_brace_fold(
        "\
foo() {
    # { this brace is ignored }
    echo hi
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 3);
}

#[test]
fn shell_brace_fold_skips_single_quoted_string() {
    let r = shell_brace_fold(
        "\
foo() {
    x='hello { world }'
    echo hi
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 3);
}

#[test]
fn shell_brace_fold_single_quote_no_escape() {
    // In single-quoted strings, backslash is literal — it does NOT escape
    // the following quote. So 'hello \\' ends at the second ', and the
    // closing brace on the same line is a real fold boundary.
    let r = shell_brace_fold(
        "\
foo() {
    x='hello \\\\'}' world'
    echo hi
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 1);
}

#[test]
fn shell_brace_fold_skips_double_quoted_string() {
    let r = shell_brace_fold(
        "\
foo() {
    x=\"hello { world }\"
    echo hi
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 3);
}

#[test]
fn shell_brace_fold_skips_escaped_quote_in_double_quoted() {
    let r = shell_brace_fold(
        "\
foo() {
    x=\"escaped \\\" quote { brace }\"
    echo hi
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 3);
}

#[test]
fn shell_brace_fold_skips_heredoc() {
    let r = shell_brace_fold(
        "\
foo() {
    cat <<EOF
{ brace in heredoc }
EOF
    echo hi
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 5);
}

#[test]
fn shell_brace_fold_skips_heredoc_with_spaces_around_delimiter() {
    let r = shell_brace_fold(
        "\
foo() {
    cat << EOF
{ brace }
EOF
    echo hi
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 5);
}

#[test]
fn shell_brace_fold_skips_indented_heredoc_delimiter() {
    // The closing delimiter may be indented with tabs (<<- style).
    let r = shell_brace_fold(
        "\
foo() {
    cat <<-EOF
{ brace }
\tEOF
    echo hi
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 5);
}

#[test]
fn shell_brace_fold_heredoc_does_not_close_on_partial_match() {
    let r = shell_brace_fold(
        "\
foo() {
    cat <<EOF
EOF is not the end
EOF
    echo hi
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 5);
}

#[test]
fn shell_brace_fold_multiple_heredocs() {
    let r = shell_brace_fold(
        "\
foo() {
    cat <<A
{ brace }
A
    cat <<B
{ brace }
B
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 7);
}

#[test]
fn shell_brace_fold_crlf() {
    let r = shell_brace_fold("foo() {\r\n    echo hi\r\n}\r\n");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 2);
}

#[test]
fn shell_brace_fold_unmatched_close_is_silent() {
    let r = shell_brace_fold("}\n{\n    echo hi\n}\n");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 1);
    assert_eq!(r[0].end, 3);
}

#[test]
fn shell_brace_fold_unmatched_open_leaves_stack() {
    let r = shell_brace_fold("{\n    echo hi\n");
    assert!(r.is_empty());
}

#[test]
fn shell_brace_fold_here_string_not_heredoc() {
    // <<< is a here-string, not a heredoc — no delimiter to match.
    let r = shell_brace_fold(
        "\
foo() {
    cat <<< \"hello { world }\"
    echo hi
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 3);
}

#[test]
fn shell_brace_fold_multiline_single_quoted_string_counts_lines() {
    // Newlines inside '…' must advance the line counter.
    let r = shell_brace_fold(
        "\
x='line one
line two'
foo() {
    echo hi
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 2);
    assert_eq!(r[0].end, 4);
}

#[test]
fn shell_brace_fold_hash_in_expansion_is_not_comment() {
    // ${#arr[@]} and $# are parameter expansions, not comments.
    let r = shell_brace_fold(
        "\
foo() {
    n=${#arr[@]}
    echo $#
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 3);
}

#[test]
fn shell_brace_fold_hash_after_separator_is_comment() {
    let r = shell_brace_fold(
        "\
foo() {
    echo hi; # trailing { comment }
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 2);
}

#[test]
fn shell_brace_fold_quoted_heredoc_delimiter() {
    // <<'EOF', <<"EOF" and <<\EOF all terminate on a bare EOF line.
    for open in ["<<'EOF'", "<<\"EOF\"", "<<\\EOF"] {
        let src = format!(
            "\
foo() {{
    cat {open}
{{ brace }}
EOF
    echo hi
}}
"
        );
        let r = shell_brace_fold(&src);
        assert_eq!(r.len(), 1, "opener {open}");
        assert_eq!(r[0].start, 0);
        assert_eq!(r[0].end, 5);
    }
}

#[test]
fn shell_brace_fold_plain_heredoc_delimiter_must_not_be_indented() {
    // Without <<-, an indented delimiter line does not close the heredoc.
    let r = shell_brace_fold(
        "\
foo() {
    cat <<EOF
\tEOF
{ brace }
EOF
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 5);
}

#[test]
fn shell_brace_fold_arithmetic_shift_is_not_heredoc() {
    let r = shell_brace_fold(
        "\
foo() {
    x=$((1<<2))
    (( y = x << 3 ))
    echo hi
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 4);
}

#[test]
fn shell_brace_fold_escaped_quote_outside_string() {
    // \' and \" must not open a string state.
    let r = shell_brace_fold(
        "\
foo() {
    echo can\\'t
    echo \\\"
    echo hi
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 4);
}

// ---------------------------------------------------------------------------
// hcl_brace_fold tests
// ---------------------------------------------------------------------------

#[test]
fn hcl_brace_fold_empty() {
    assert!(hcl_brace_fold("").is_empty());
}

#[test]
fn hcl_brace_fold_no_braces() {
    assert!(hcl_brace_fold("ami = \"ami-12345\"\n").is_empty());
}

#[test]
fn hcl_brace_fold_single_line_block() {
    // Single-line {…} — no region (must span >1 line).
    assert!(hcl_brace_fold("provider \"aws\" { region = \"us-east-1\" }\n").is_empty());
}

#[test]
fn hcl_brace_fold_nested_resource_blocks() {
    let r = hcl_brace_fold(
        "\
resource \"aws_instance\" \"web\" {
  ami           = \"ami-12345\"
  instance_type = \"t2.micro\"

  tags = {
    Name = \"web\"
  }
}
",
    );
    assert_eq!(r.len(), 2);
    // Inner tags block closes first.
    assert_eq!(r[0].start, 4);
    assert_eq!(r[0].end, 6);
    // Outer resource block closes last.
    assert_eq!(r[1].start, 0);
    assert_eq!(r[1].end, 7);
}

#[test]
fn hcl_brace_fold_skips_hash_comment() {
    // Braces inside `#` comments must not be treated as fold boundaries.
    let r = hcl_brace_fold(
        "\
resource \"aws_instance\" \"web\" {
  # startup script body { not a block }
  ami = \"ami-12345\"
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 3);
}

#[test]
fn hcl_brace_fold_skips_slash_comment() {
    let r = hcl_brace_fold(
        "\
resource \"aws_instance\" \"web\" {
  // { ignored }
  ami = \"ami-12345\"
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 3);
}

#[test]
fn hcl_brace_fold_skips_block_comment() {
    let r = hcl_brace_fold(
        "\
resource \"aws_instance\" \"web\" {
  /*
    { nested brace in block comment }
  */
  ami = \"ami-12345\"
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 5);
}

#[test]
fn hcl_brace_fold_skips_double_quoted_string() {
    // HCL string interpolation carries braces inside the quotes.
    let r = hcl_brace_fold(
        "\
resource \"aws_bucket\" \"logs\" {
  name = \"logs-${var.env}-{not-a-block}\"
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 2);
}

#[test]
fn hcl_brace_fold_skips_heredoc() {
    let r = hcl_brace_fold(
        "\
resource \"aws_instance\" \"web\" {
  user_data = <<-EOF
    #!/bin/bash
    echo \"{ not a block }\"
    for i in {1..3}; do echo $i; done
  EOF
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 6);
}

#[test]
fn hcl_brace_fold_skips_quoted_delimiter_heredoc() {
    let r = hcl_brace_fold(
        "\
locals {
  script = <<\"EOT\"
value = { \"ignored\": true }
EOT
}
",
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 4);
}

#[test]
fn hcl_brace_fold_unclosed_brace() {
    let r = hcl_brace_fold("resource \"x\" \"y\" {\n  count = 1\n");
    assert!(r.is_empty());
}

// ---------------------------------------------------------------------------
// yaml_fold tests
// ---------------------------------------------------------------------------

#[test]
fn yaml_fold_empty() {
    let empty: &[&str] = &[];
    assert!(yaml_fold(empty).is_empty());
}

#[test]
fn yaml_fold_delegates_to_yaml_fold_detect_fold_regions() {
    // fold_detectors::yaml_fold is a thin delegate to the built-in
    // crate::yaml_fold::detect_fold_regions — same algorithm, same output,
    // reachable from the bundled `yaml` plugin (`plugins/yaml`).
    let ls: Vec<&str> = "outer:\n  inner: 1\n  other: 2\nflat: 3".lines().collect();
    let r = yaml_fold(&ls);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 2);

    let via_detect_fold_regions = crate::yaml_fold::detect_fold_regions(&ls);
    assert_eq!(via_detect_fold_regions.len(), r.len());
    assert_eq!(via_detect_fold_regions[0].start, r[0].start);
    assert_eq!(via_detect_fold_regions[0].end, r[0].end);
}
