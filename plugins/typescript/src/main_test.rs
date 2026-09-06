use super::*;

#[test]
fn registers_typescript_and_javascript_extensions() {
    let mut output = Vec::new();
    register_language_provider(&mut output);
    let msg: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(msg["params"]["extensions"], serde_json::json!(EXTENSIONS));
    assert_eq!(msg["params"]["capabilities"][0], "fold");
}

#[test]
fn folds_ts_interfaces_classes_async_and_arrow_functions() {
    let path = std::env::temp_dir().join("mantis-typescript-fold-test.tsx");
    std::fs::write(&path, "interface View {\n  value: string;\n}\nclass App {\n  async run() {\n    const f = () => {\n      return <View />;\n    };\n  }\n}\n").unwrap();
    let mut output = Vec::new();
    handle_open(path.to_str().unwrap(), &mut output);
    let msg: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(
        msg["params"]["regions"],
        serde_json::json!([[0, 2], [5, 7], [4, 8], [3, 9]])
    );
    std::fs::remove_file(path).ok();
}
