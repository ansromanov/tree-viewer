use super::*;

#[test]
fn registers_toml_fold_provider() {
    let mut output = Vec::new();
    register_language_provider(&mut output);
    let msg: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(msg["params"]["extensions"][0], "toml");
    assert_eq!(msg["params"]["capabilities"][0], "fold");
}

#[test]
fn folds_tables_and_array_tables() {
    let path = std::env::temp_dir().join("mantis-toml-fold-test.toml");
    std::fs::write(
        &path,
        "title = \"demo\"\n[package]\nname = \"x\"\n[[bin]]\nname = \"x\"\n",
    )
    .unwrap();
    let mut output = Vec::new();
    handle_open(path.to_str().unwrap(), &mut output);
    let msg: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(
        msg["params"]["regions"],
        serde_json::json!([[1, 2], [3, 4]])
    );
    std::fs::remove_file(path).ok();
}
