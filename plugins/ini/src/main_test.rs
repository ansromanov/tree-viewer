use super::*;

#[test]
fn registers_ini_family_extensions() {
    let mut output = Vec::new();
    register_language_provider(&mut output);
    let msg: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(
        msg["params"]["extensions"],
        serde_json::json!([
            "ini",
            "service",
            "timer",
            "conf",
            "properties",
            "cfg",
            "desktop"
        ])
    );
}

#[test]
fn folds_systemd_sections_without_trailing_comments() {
    let path = std::env::temp_dir().join("mantis-ini-fold-test.service");
    std::fs::write(
        &path,
        "[Unit]\nDescription=demo\n\n# end\n[Service]\nExecStart=demo\n",
    )
    .unwrap();
    let mut output = Vec::new();
    handle_open(path.to_str().unwrap(), &mut output);
    let msg: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(
        msg["params"]["regions"],
        serde_json::json!([[0, 1], [4, 5]])
    );
    std::fs::remove_file(path).ok();
}
