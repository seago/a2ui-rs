//! CLI 进程级行为测试：
//! 1. stdout 是 JSONL 协议输出通道，不得被日志/欢迎信息污染；
//! 2. stdin 保持打开时不得因空闲超时被误判为 EOF 而退出。

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

const CAPABILITIES_LINE: &str =
    r#"{"version":"v1.0","capabilities":{"version":"1.0","features":["basic"]}}"#;

fn spawn_render() -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_a2ui"))
        .arg("render")
        .env("RUST_LOG", "info")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn a2ui binary")
}

#[test]
fn stdout_carries_only_jsonl_protocol_messages() {
    let mut child = spawn_render();
    let mut stdin = child.stdin.take().expect("child stdin");
    writeln!(stdin, "{CAPABILITIES_LINE}").unwrap();
    writeln!(
        stdin,
        r#"{{"version":"v1.0","createSurface":{{"surfaceId":"s1","catalogId":"basic"}}}}"#
    )
    .unwrap();
    drop(stdin); // EOF

    let output = child.wait_with_output().expect("wait for child");
    let stdout = String::from_utf8_lossy(&output.stdout);

    // stdout 上的每个非空行都必须是合法 JSON（协议信封）；
    // 日志与人类可读提示只允许出现在 stderr
    for line in stdout.lines().filter(|l| !l.trim().is_empty()) {
        assert!(
            line.parse::<a2ui_core::Value>().is_ok(),
            "stdout protocol channel polluted with non-JSON content: {line:?}"
        );
    }
}

#[test]
fn processes_message_arriving_after_idle_period() {
    let mut child = spawn_render();
    let mut stdin = child.stdin.take().expect("child stdin");
    writeln!(stdin, "{CAPABILITIES_LINE}").unwrap();
    stdin.flush().unwrap();

    // 对端（LLM Agent）思考几秒不发消息是常态；管道未关闭就不是 EOF，
    // 空闲期之后到达的消息必须仍被处理
    std::thread::sleep(Duration::from_secs(6));
    writeln!(
        stdin,
        r#"{{"version":"v1.0","createSurface":{{"surfaceId":"late_surface","catalogId":"basic"}}}}"#
    )
    .unwrap();
    drop(stdin); // EOF

    let output = child.wait_with_output().expect("wait for child");
    assert!(
        output.status.success(),
        "expected clean exit, got {}",
        output.status
    );

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("late_surface"),
        "message sent after a 6s idle period was never processed — idle timeout misread as EOF"
    );
}
