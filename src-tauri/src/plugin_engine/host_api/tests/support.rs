use super::*;

#[test]
fn expand_path_expands_tilde_prefix() {
    let home = dirs::home_dir().expect("home dir");
    let expected = home.join(".claude-custom").to_string_lossy().to_string();

    assert_eq!(expand_path("~/.claude-custom"), expected);
}
