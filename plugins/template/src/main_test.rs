use super::*;

#[test]
fn registers_template_extensions() {
    let mut output = Vec::new();
    register_language_provider(&mut output);
    let msg: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(msg["params"]["extensions"], serde_json::json!(EXTENSIONS));
}

#[test]
fn folds_helm_define_and_nested_jinja_blocks() {
    let path = std::env::temp_dir().join("mantis-template-fold-test.tpl");
    std::fs::write(
        &path,
        "{{ define \"labels\" }}\n{% if enabled %}\nvalue\n{% endif %}\n{{ end }}\n",
    )
    .unwrap();
    let mut output = Vec::new();
    handle_open(path.to_str().unwrap(), &mut output);
    let msg: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(
        msg["params"]["regions"],
        serde_json::json!([[1, 3], [0, 4]])
    );
    std::fs::remove_file(path).ok();
}
