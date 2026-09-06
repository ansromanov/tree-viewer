use super::*;

#[test]
fn test_register_language_provider() {
    let mut buf = Vec::new();
    register_language_provider(&mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["event"], "action");
    assert_eq!(parsed["action"], "register_language_provider");
    assert_eq!(parsed["params"]["extensions"][0], "tf");
    assert_eq!(parsed["params"]["extensions"][1], "tfvars");
    assert_eq!(parsed["params"]["extensions"][2], "hcl");
    assert_eq!(parsed["params"]["capabilities"][0], "fold");
}

#[test]
fn test_fold_regions_ok() {
    assert!(fold_regions_ok("tf"));
    assert!(fold_regions_ok("tfvars"));
    assert!(fold_regions_ok("hcl"));
    assert!(!fold_regions_ok("rs"));
    assert!(!fold_regions_ok(""));
}

#[test]
fn test_send_set_fold_regions() {
    let regions = vec![mantis::fold::FoldRegion { start: 1, end: 3 }];
    let mut buf = Vec::new();
    send_set_fold_regions(&regions, "/path/to/main.tf", &mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["event"], "action");
    assert_eq!(parsed["action"], "set_fold_regions");
    assert_eq!(parsed["params"]["path"], "/path/to/main.tf");
    assert_eq!(parsed["params"]["regions"][0][0], 1);
    assert_eq!(parsed["params"]["regions"][0][1], 3);
}

#[test]
fn test_handle_file_open_nested_blocks_and_hash_comments() {
    // Fixture: nested resource blocks with `#` comments and a heredoc that
    // contains braces which must not be treated as fold boundaries.
    let mut tmp = std::env::temp_dir();
    tmp.push("mantis_plugin_terraform_test.tf");
    std::fs::write(
        &tmp,
        "resource \"aws_instance\" \"web\" {\n  # comment { not a block }\n  ami = \"ami-1\"\n\n  user_data = <<-EOF\n    echo \"{ shell brace }\"\n  EOF\n\n  tags = {\n    Name = \"web\"\n  }\n}\n",
    )
    .unwrap();
    let path_str = tmp.to_str().unwrap().to_string();

    let mut buf = Vec::new();
    handle_file_open(&path_str, &mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["event"], "action");
    assert_eq!(parsed["action"], "set_fold_regions");
    assert_eq!(parsed["params"]["path"], path_str);
    let regions = parsed["params"]["regions"].as_array().unwrap();
    // Internal tags block (lines 8..10) closes first, then the resource block
    // (lines 0..11). The `#` comment and heredoc braces are ignored.
    assert_eq!(regions.len(), 2);
    assert_eq!(regions[0][0], 8);
    assert_eq!(regions[0][1], 10);
    assert_eq!(regions[1][0], 0);
    assert_eq!(regions[1][1], 11);

    std::fs::remove_file(&tmp).ok();
}

#[test]
fn test_handle_file_open_hcl_extension() {
    let mut tmp = std::env::temp_dir();
    tmp.push("mantis_plugin_terraform_test.hcl");
    std::fs::write(&tmp, "\"openbao\" \"path\" {\n  value = \"x\"\n}\n").unwrap();
    let path_str = tmp.to_str().unwrap().to_string();

    let mut buf = Vec::new();
    handle_file_open(&path_str, &mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["action"], "set_fold_regions");
    let regions = parsed["params"]["regions"].as_array().unwrap();
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0][0], 0);
    assert_eq!(regions[0][1], 2);

    std::fs::remove_file(&tmp).ok();
}

#[test]
fn test_handle_file_open_ignores_non_hcl() {
    // Files outside the provider's extensions must produce no output.
    let mut tmp = std::env::temp_dir();
    tmp.push("mantis_plugin_terraform_test.rs");
    std::fs::write(&tmp, "fn main() {\n    println!(\"hi\");\n}\n").unwrap();
    let path_str = tmp.to_str().unwrap().to_string();

    let mut buf = Vec::new();
    handle_file_open(&path_str, &mut buf);
    assert!(
        buf.is_empty(),
        "should not emit set_fold_regions for non-HCL"
    );

    std::fs::remove_file(&tmp).ok();
}

#[test]
fn test_handle_file_open_missing_file() {
    let mut buf = Vec::new();
    handle_file_open("/nonexistent/path/main.tf", &mut buf);
    assert!(buf.is_empty());
}
