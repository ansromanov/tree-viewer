use super::*;

#[test]
fn registers_sql_fold_provider() {
    let mut output = Vec::new();
    register_language_provider(&mut output);
    let msg: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(msg["params"]["extensions"], serde_json::json!(["sql"]));
    assert_eq!(msg["params"]["capabilities"], serde_json::json!(["fold"]));
}

#[test]
fn folds_multiline_sql_statement() {
    let path = std::env::temp_dir().join("mantis-sql-fold-test.sql");
    std::fs::write(&path, "CREATE TABLE users (\n  id INTEGER\n);\n").unwrap();
    let mut output = Vec::new();
    handle_open(path.to_str().unwrap(), &mut output);
    let msg: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(msg["params"]["regions"], serde_json::json!([[0, 2]]));
    std::fs::remove_file(path).ok();
}
