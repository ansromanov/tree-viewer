use super::*;

#[test]
fn registers_css_family_fold_provider() {
    let mut output = Vec::new();
    register_language_provider(&mut output);
    let msg: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(
        msg["params"]["extensions"],
        serde_json::json!(["css", "scss", "less"])
    );
}

#[test]
fn folds_nested_rules_and_media_query() {
    let path = std::env::temp_dir().join("mantis-css-fold-test.css");
    std::fs::write(
        &path,
        "root {\n  color: red;\n}\n@media screen {\n  .x {\n    color: blue;\n  }\n}\n",
    )
    .unwrap();
    let mut output = Vec::new();
    handle_open(path.to_str().unwrap(), &mut output);
    let msg: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(
        msg["params"]["regions"],
        serde_json::json!([[0, 2], [4, 6], [3, 7]])
    );
    std::fs::remove_file(path).ok();
}
