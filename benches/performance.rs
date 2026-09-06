// Run: cargo bench [-- "filter"]

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use mantis::app::App;
use mantis::config::{Config, ContentConfig, TreeConfig};
use mantis::highlight::Highlighter;
use mantis::search::SearchState;
use mantis::tree::{build_visible, collect_all_files};
use mantis::virtual_file::VirtualFile;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

// ---------------------------------------------------------------------------
// Benchmarks: event parser — exercises the code paths most likely to have
// correctness regressions (kitty CSI-u, tilde, arrows, plain ASCII).
// ---------------------------------------------------------------------------

fn bench_event_parser(c: &mut Criterion) {
    use mantis::event_source::parser::parse_event;

    let csi_u = b"\x1b[1079:1047:112;1u";
    let csi_tilde = b"\x1b[5;5~";
    let csi_arrow = b"\x1b[A";
    let plain_ascii = b"hello world";
    let utf8_multi = "héllo мир 👋".as_bytes();

    let mut group = c.benchmark_group("event_parser");
    group.throughput(criterion::Throughput::Bytes(1));

    group.bench_with_input(BenchmarkId::new("csi_u", ""), csi_u, |b, input| {
        b.iter(|| black_box(parse_event(black_box(input))))
    });
    group.bench_with_input(BenchmarkId::new("csi_tilde", ""), csi_tilde, |b, input| {
        b.iter(|| black_box(parse_event(black_box(input))))
    });
    group.bench_with_input(BenchmarkId::new("csi_arrow", ""), csi_arrow, |b, input| {
        b.iter(|| black_box(parse_event(black_box(input))))
    });
    group.bench_with_input(
        BenchmarkId::new("plain_ascii", ""),
        plain_ascii,
        |b, input| b.iter(|| black_box(parse_event(black_box(input)))),
    );
    group.bench_with_input(
        BenchmarkId::new("utf8_multi", ""),
        utf8_multi,
        |b, input| b.iter(|| black_box(parse_event(black_box(input)))),
    );

    group.finish();
}

// ---------------------------------------------------------------------------
// Counter for unique temp dir names
// ---------------------------------------------------------------------------

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn fixture_dir(label: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("mantis_bench_{}_{}", label, n));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

// ---------------------------------------------------------------------------
// Fixture generators
// ---------------------------------------------------------------------------

/// `count` files (~50 bytes each) in a flat directory.
fn generate_many_files(dir: &Path, count: usize) {
    fs::create_dir_all(dir).unwrap();
    for i in 0..count {
        let content =
            format!("// file {i}\npub fn example() -> i32 {{\n    let x = {i};\n    x\n}}\n");
        fs::write(dir.join(format!("f{i:05}.rs")), &content).unwrap();
    }
}

/// A single `.rs` file with `line_count` lines of realistic-ish Rust code.
fn generate_large_rs_file(path: &Path, line_count: usize) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut content = String::with_capacity(line_count * 50);
    for i in 0..line_count {
        match i % 5 {
            0 => content.push_str(&format!("// line {i} — a comment\n")),
            1 => content.push_str(&format!("pub fn func_{i}() -> i32 {{ {i} }}\n")),
            2 => content.push_str(&format!("let x_{i} = \"hello world {i}\";\n")),
            3 => content.push_str(&format!("if x_{i} > 0 {{ println!(\"{{}}\", x_{i}); }}\n")),
            _ => content.push_str(&format!("/* multi\nline\ncomment {i} */\n")),
        }
    }
    fs::write(path, content).unwrap();
}

/// `dirs` sibling subdirectories (collapsed by default) each holding
/// `files` small files. Used to exercise full-tree matching over entries
/// that are not in the visible node list.
fn generate_nested_files(dir: &Path, dirs: usize, files: usize) {
    for d in 0..dirs {
        let sub = dir.join(format!("sub{d:03}"));
        fs::create_dir_all(&sub).unwrap();
        for i in 0..files {
            fs::write(sub.join(format!("f{i:04}.rs")), "fn f() {}\n").unwrap();
        }
    }
}

/// A deep directory tree: d0/d1/d2/.../f{i}.rs at each level.
fn generate_deep_tree(dir: &Path, depth: usize) {
    let mut current = dir.to_path_buf();
    for i in 0..depth {
        current = current.join(format!("d{i}"));
        fs::create_dir_all(&current).unwrap();
        fs::write(current.join(format!("f{i}.rs")), "fn f() {}\n").unwrap();
    }
}

/// Generates searchable text files with `count` files each having `lines` lines.
fn generate_search_files(dir: &Path, count: usize, lines: usize) {
    fs::create_dir_all(dir).unwrap();
    for i in 0..count {
        let mut content = String::with_capacity(lines * 40);
        for j in 0..lines {
            content.push_str(&format!(
                "line {j} of file {i}: the quick brown fox jumps over the lazy dog\n"
            ));
        }
        fs::write(dir.join(format!("s{i:05}.txt")), &content).unwrap();
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Recursively visit every directory so the whole tree is visible.
fn expand_all(root: &Path) -> HashSet<PathBuf> {
    let mut expanded = HashSet::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if expanded.insert(dir.clone()) {
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        stack.push(entry.path());
                    }
                }
            }
        }
    }
    expanded
}

fn highlighter() -> Highlighter {
    Highlighter::with_extra_syntaxes("base16-ocean.dark", &[])
}

// ---------------------------------------------------------------------------
// Benchmark: tree walk — build_visible + collect_all_files
// ---------------------------------------------------------------------------

fn bench_tree_walk(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_walk");

    for &count in &[100usize, 1_000] {
        let dir = fixture_dir("tree_flat");
        generate_many_files(&dir, count);
        let root = dir.canonicalize().unwrap();
        let expanded = expand_all(&root);

        group.bench_with_input(
            BenchmarkId::new("build_visible/flat", count),
            &(&root, &expanded),
            |b, (root, expanded)| {
                let deleted: HashSet<PathBuf> = HashSet::new();
                b.iter(|| black_box(build_visible(root, expanded, false, true, &deleted)))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("collect_all_files/flat", count),
            &root,
            |b, root| b.iter(|| black_box(collect_all_files(root, false, true))),
        );

        fs::remove_dir_all(&dir).ok();
    }

    for &depth in &[10usize, 50] {
        let dir = fixture_dir("tree_deep");
        generate_deep_tree(&dir, depth);
        let root = dir.canonicalize().unwrap();
        let expanded = expand_all(&root);

        group.bench_with_input(
            BenchmarkId::new("build_visible/deep", depth),
            &(&root, &expanded),
            |b, (root, expanded)| {
                let deleted: HashSet<PathBuf> = HashSet::new();
                b.iter(|| black_box(build_visible(root, expanded, false, true, &deleted)))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("collect_all_files/deep", depth),
            &root,
            |b, root| b.iter(|| black_box(collect_all_files(root, false, true))),
        );

        fs::remove_dir_all(&dir).ok();
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: content search per keystroke (cache-hit path)
// ---------------------------------------------------------------------------

fn bench_content_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("content_search");

    for &(files, lines_per_file) in &[(10usize, 100), (100, 100)] {
        let dir = fixture_dir("search");
        generate_search_files(&dir, files, lines_per_file);

        group.bench_with_input(
            BenchmarkId::new(
                "refresh_content/warm",
                format!("{files}fx{lines_per_file}l"),
            ),
            &dir,
            |b, dir| {
                let mut state = SearchState::new(dir, false, true, 0, None);
                state.toggle_mode();
                // Warm the cache
                state.push('x');
                state.push('y');
                state.refresh_now();
                state.pop();
                state.pop();

                b.iter(|| {
                    state.push('b');
                    state.push('r'); // 2-char query to trigger content search
                    state.refresh_now();
                    state.pop();
                    state.pop();
                    state.refresh_now();
                })
            },
        );

        fs::remove_dir_all(&dir).ok();
    }

    // Regex + whole-word variant: exercises the compiled-regex matching path
    // added for the search-option toggles.
    {
        let (files, lines_per_file) = (100usize, 100);
        let dir = fixture_dir("search_regex");
        generate_search_files(&dir, files, lines_per_file);

        group.bench_with_input(
            BenchmarkId::new(
                "refresh_content/regex_whole_word",
                format!("{files}fx{lines_per_file}l"),
            ),
            &dir,
            |b, dir| {
                let mut state = SearchState::new(dir, false, true, 0, None);
                state.toggle_mode();
                state.regex = true;
                state.whole_word = true;
                // Warm the cache
                state.push('x');
                state.push('y');
                state.refresh_now();
                state.pop();
                state.pop();

                b.iter(|| {
                    state.push('b');
                    state.push('r'); // 2-char query to trigger content search
                    state.refresh_now();
                    state.pop();
                    state.pop();
                    state.refresh_now();
                })
            },
        );

        fs::remove_dir_all(&dir).ok();
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: VirtualFile open
// ---------------------------------------------------------------------------

fn bench_file_open(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_open");

    for &lines in &[1_000, 10_000, 100_000] {
        let dir = fixture_dir("open");
        let path = dir.join("large.rs");
        generate_large_rs_file(&path, lines);

        group.bench_with_input(
            BenchmarkId::new("VirtualFile::open", lines),
            &path,
            |b, path| b.iter(|| black_box(VirtualFile::open(path))),
        );

        fs::remove_dir_all(&dir).ok();
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: syntax highlight
// ---------------------------------------------------------------------------

fn bench_highlight(c: &mut Criterion) {
    let mut group = c.benchmark_group("highlight");
    let hl = highlighter();

    for &lines in &[100, 1_000, 10_000] {
        let dir = fixture_dir("highlight");
        let path = dir.join("large.rs");
        generate_large_rs_file(&path, lines);

        let vf = VirtualFile::open(&path).unwrap();
        let all_lines: Vec<String> = (0..vf.line_count())
            .filter_map(|i| vf.line_text(i).map(String::from))
            .collect();

        group.bench_with_input(
            BenchmarkId::new("highlight/all", lines),
            &(&path, &all_lines),
            |b, (path, lines)| b.iter(|| black_box(hl.highlight(path, lines))),
        );

        // Simulate a scrolling window of 50 lines
        let window = 50usize;
        if all_lines.len() > window {
            let syntax_name = hl.syntax_name(&path);
            let mid = all_lines.len() / 2;
            let window_slice: Vec<&str> = all_lines[mid..mid + window]
                .iter()
                .map(String::as_str)
                .collect();

            group.bench_with_input(
                BenchmarkId::new("highlight_range/50_window", lines),
                &window_slice,
                |b, slice| b.iter(|| black_box(hl.highlight_range(syntax_name.as_deref(), slice))),
            );
        }

        fs::remove_dir_all(&dir).ok();
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: scroll / redraw — highlight a visible window at different
// positions in a large file.
// ---------------------------------------------------------------------------

fn bench_scroll_redraw(c: &mut Criterion) {
    let mut group = c.benchmark_group("scroll_redraw");
    let hl = highlighter();
    let file_lines = 10_000;

    let dir = fixture_dir("scroll");
    let path = dir.join("large.rs");
    generate_large_rs_file(&path, file_lines);

    let vf = VirtualFile::open(&path).unwrap();
    let all_lines: Vec<String> = (0..vf.line_count())
        .filter_map(|i| vf.line_text(i).map(String::from))
        .collect();
    let refs: Vec<&str> = all_lines.iter().map(String::as_str).collect();
    let syntax_name = hl.syntax_name(&path);

    for &ws in &[25usize, 50] {
        // Top of file
        let top: Vec<&str> = refs[..ws].to_vec();
        group.bench_with_input(BenchmarkId::new("highlight_range/top", ws), &top, |b, w| {
            b.iter(|| black_box(hl.highlight_range(syntax_name.as_deref(), w)))
        });

        // Middle
        let mid = refs.len() / 2;
        let middle: Vec<&str> = refs[mid..mid + ws].to_vec();
        group.bench_with_input(
            BenchmarkId::new("highlight_range/mid", ws),
            &middle,
            |b, w| b.iter(|| black_box(hl.highlight_range(syntax_name.as_deref(), w))),
        );

        // End
        let end: Vec<&str> = refs[refs.len() - ws..].to_vec();
        group.bench_with_input(BenchmarkId::new("highlight_range/end", ws), &end, |b, w| {
            b.iter(|| black_box(hl.highlight_range(syntax_name.as_deref(), w)))
        });
    }

    fs::remove_dir_all(&dir).ok();
    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: tree redraw — draw_tree with guides on/off, filter active
// ---------------------------------------------------------------------------

fn bench_tree_redraw(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_redraw");

    for &node_count in &[1_000usize, 5_000] {
        let dir = fixture_dir("tree_redraw");
        generate_many_files(&dir, node_count);
        let root = dir.canonicalize().unwrap();

        let cfg = Config {
            tree: TreeConfig {
                indent_guides: false,
                icons: false,
                show_hidden: false,
                ..Default::default()
            },
            content: ContentConfig {
                scrollbar: false,
                line_numbers: false,
                ..Default::default()
            },
            ..Config::default()
        };
        // generate_many_files creates a flat directory; App::new shows all files
        // at depth 1 under the root (which is expanded by default).
        let mut app = App::new(root.clone(), cfg, None, None).unwrap();

        let mut terminal = Terminal::new(TestBackend::new(80, 40)).unwrap();
        let area = Rect::new(0, 0, 25, 38);

        // Guides off (most common case)
        app.indent_guides = false;
        group.bench_with_input(
            BenchmarkId::new("draw/no_guides", node_count),
            &(),
            |b, _| {
                b.iter(|| {
                    terminal
                        .draw(|f| mantis::ui::tree::draw_tree(f, &mut app, area))
                        .unwrap();
                    black_box(&app.tree_area);
                })
            },
        );

        // Guides on
        app.indent_guides = true;
        group.bench_with_input(
            BenchmarkId::new("draw/with_guides", node_count),
            &(),
            |b, _| {
                b.iter(|| {
                    terminal
                        .draw(|f| mantis::ui::tree::draw_tree(f, &mut app, area))
                        .unwrap();
                    black_box(&app.tree_area);
                })
            },
        );

        // Filter active (guides off)
        app.indent_guides = false;
        app.tree_filter = Some(mantis::search::TreeFilter::new());
        app.tree_filter.as_mut().unwrap().push('f');
        group.bench_with_input(
            BenchmarkId::new("draw/with_filter", node_count),
            &(),
            |b, _| {
                b.iter(|| {
                    terminal
                        .draw(|f| mantis::ui::tree::draw_tree(f, &mut app, area))
                        .unwrap();
                    black_box(&app.tree_area);
                })
            },
        );

        // Clean up fixture.
        app.tree_filter = None;
        fs::remove_dir_all(&dir).ok();
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// 7. Tree-filter keystroke: full-tree match + auto-expand sync
// ---------------------------------------------------------------------------

fn bench_tree_filter_sync(c: &mut Criterion) {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut group = c.benchmark_group("tree_filter_sync");

    for &(dirs, files) in &[(50usize, 100usize), (100, 200)] {
        let total = dirs * files;
        let dir = fixture_dir("filter_sync");
        generate_nested_files(&dir, dirs, files);

        let mut app = App::new(dir.clone(), Config::default(), None, None).unwrap();

        // Open the filter and type the first char outside the measured loop so
        // the session's full-tree path cache exists and every subdirectory is
        // already auto-expanded; each iteration then measures the steady-state
        // per-keystroke cost (match scan, no rebuild).
        app.tree_filter = Some(mantis::search::TreeFilter::new());
        app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::empty()));

        group.bench_with_input(BenchmarkId::new("keystroke", total), &(), |b, _| {
            b.iter(|| {
                app.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty()));
                app.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
                black_box(&app.expanded);
            })
        });

        fs::remove_dir_all(&dir).ok();
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: brace_fold — large synthetic source with nested blocks
// ---------------------------------------------------------------------------

fn generate_brace_source(line_count: usize) -> String {
    let mut content = String::with_capacity(line_count * 40);
    // Top-level function for every 10 lines
    for chunk in 0..line_count / 10 {
        content.push_str(&format!("fn func_{chunk}() {{\n"));
        for i in 0..9 {
            match i % 3 {
                0 => content.push_str(&format!("    let x_{i} = {i};\n")),
                1 => content.push_str(&format!("    if x_{i} > 0 {{ process(x_{i}); }}\n")),
                _ => content.push_str("    // comment with }\n"),
            }
        }
        content.push_str("}\n\n");
    }
    content
}

fn generate_indent_source(line_count: usize) -> String {
    let mut content = String::with_capacity(line_count * 40);
    // A top-level class with many methods
    content.push_str("class App:\n");
    for i in 0..line_count.saturating_sub(1) / 20 {
        content.push_str(&format!("    def method_{i}(self):\n"));
        for j in 0..19 {
            match j % 4 {
                0 => content.push_str(&format!("        x_{j} = {j}\n")),
                1 => content.push_str(&format!("        if x_{j} > 0:\n")),
                2 => content.push_str(&format!("            result = x_{j} * 2\n")),
                _ => content.push_str("        return result\n"),
            }
        }
    }
    content
}

fn bench_brace_fold(c: &mut Criterion) {
    let mut group = c.benchmark_group("brace_fold");
    for &lines in &[100, 1_000, 10_000] {
        let src = generate_brace_source(lines);
        group.bench_with_input(BenchmarkId::new("brace_fold", lines), &src, |b, src| {
            b.iter(|| black_box(mantis::fold_detectors::brace_fold(black_box(src))))
        });
    }
    group.finish();
}

fn bench_indent_fold(c: &mut Criterion) {
    let mut group = c.benchmark_group("indent_fold");
    for &lines in &[100, 1_000, 10_000] {
        let src = generate_indent_source(lines);
        group.bench_with_input(BenchmarkId::new("indent_fold", lines), &src, |b, src| {
            b.iter(|| black_box(mantis::fold_detectors::indent_fold(black_box(src))))
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: brace_fold_with_brackets — large synthetic pretty-printed JSON
// ---------------------------------------------------------------------------

fn generate_json_source(line_count: usize) -> String {
    // An array of objects, each with a nested array — mirrors what
    // `serde_json::to_string_pretty` produces, which is what the `json`
    // plugin actually folds.
    let mut content = String::with_capacity(line_count * 24);
    content.push_str("[\n");
    for i in 0..line_count / 6 {
        content.push_str("  {\n");
        content.push_str(&format!("    \"id\": {i},\n"));
        content.push_str(&format!("    \"name\": \"item_{i}\",\n"));
        content.push_str("    \"tags\": [\n      \"a\",\n      \"b\"\n    ]\n");
        content.push_str("  },\n");
    }
    content.push_str("]\n");
    content
}

fn bench_brace_fold_with_brackets(c: &mut Criterion) {
    let mut group = c.benchmark_group("brace_fold_with_brackets");
    for &lines in &[100, 1_000, 10_000] {
        let src = generate_json_source(lines);
        group.bench_with_input(
            BenchmarkId::new("brace_fold_with_brackets", lines),
            &src,
            |b, src| {
                b.iter(|| {
                    black_box(mantis::fold_detectors::brace_fold_with_brackets(black_box(
                        src,
                    )))
                })
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: hcl_brace_fold — synthetic Terraform with resources, blocks,
// comments, strings, and heredocs
// ---------------------------------------------------------------------------

fn generate_hcl_source(line_count: usize) -> String {
    let mut content = String::with_capacity(line_count * 40);
    // A resource block for every 12 lines
    for chunk in 0..line_count / 12 {
        content.push_str(&format!("resource \"aws_instance\" \"box_{chunk}\" {{\n"));
        for i in 0..11 {
            match i % 4 {
                0 => content.push_str(&format!("    ami         = \"ami-{i}\"\n")),
                1 => content.push_str(&format!("    count       = {i}\n")),
                2 => content.push_str("    # comment with }\n"),
                _ => content.push_str("    user_data = <<-EOF\n      echo hi\n    EOF\n"),
            }
        }
        content.push_str("}\n\n");
    }
    // Tail: a top-level variable block plus an unterminated closing brace to
    // exercise unbalanced-input handling.
    if !line_count.is_multiple_of(12) {
        content.push_str("variable \"tail\" {\n  type = string\n}\n");
    }
    content.push_str("}\n");
    content
}

fn bench_hcl_brace_fold(c: &mut Criterion) {
    let mut group = c.benchmark_group("hcl_brace_fold");
    for &lines in &[100, 1_000, 10_000] {
        let src = generate_hcl_source(lines);
        group.bench_with_input(BenchmarkId::new("hcl_brace_fold", lines), &src, |b, src| {
            b.iter(|| black_box(mantis::fold_detectors::hcl_brace_fold(black_box(src))))
        });
    }
    group.finish();
}

fn bench_telemetry_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("telemetry_overhead");

    let tel_disabled = mantis::telemetry::Telemetry::new(false);
    group.bench_function(BenchmarkId::new("disabled", ""), |b| {
        b.iter(|| {
            tel_disabled.record(black_box(
                mantis::telemetry::TelemetryEvent::ActionInvoked {
                    action: "test_action",
                    source: mantis::telemetry::ActionSource::Key,
                },
            ));
        })
    });

    let tel_enabled = mantis::telemetry::Telemetry::new(true);
    group.bench_function(BenchmarkId::new("enabled", ""), |b| {
        b.iter(|| {
            tel_enabled.record(black_box(
                mantis::telemetry::TelemetryEvent::ActionInvoked {
                    action: "test_action",
                    source: mantis::telemetry::ActionSource::Key,
                },
            ));
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Criterion entry point
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_event_parser,
    bench_tree_walk,
    bench_content_search,
    bench_file_open,
    bench_highlight,
    bench_scroll_redraw,
    bench_tree_redraw,
    bench_tree_filter_sync,
    bench_brace_fold,
    bench_indent_fold,
    bench_brace_fold_with_brackets,
    bench_hcl_brace_fold,
    bench_telemetry_overhead,
);

criterion_main!(benches);
