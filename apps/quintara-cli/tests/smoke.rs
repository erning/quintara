//! CLI 冒烟：`quintara match` 跑一局内置 bot 对局应成功并打出结果。
#![allow(clippy::unwrap_used)]

use std::process::Command;

#[test]
fn bot_vs_bot_match_runs_and_reports_result() {
    let exe = env!("CARGO_BIN_EXE_quintara");
    let output = Command::new(exe)
        .args([
            "match",
            "--player",
            "builtin:random",
            "--player",
            "builtin:greedy",
            "--rule",
            "freestyle",
            "--size",
            "15",
            "--quiet",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "non-zero exit");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("wins") || stdout.contains("Draw"),
        "expected a result line, got: {stdout}"
    );
}

#[test]
fn rejects_wrong_player_count() {
    let exe = env!("CARGO_BIN_EXE_quintara");
    let output = Command::new(exe)
        .args(["match", "--player", "builtin:random"])
        .output()
        .unwrap();
    assert!(!output.status.success(), "should fail with one player");
}
