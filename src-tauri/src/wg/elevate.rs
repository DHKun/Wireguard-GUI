//! 特权操作桥接：所有需要 root 的 wg/wg-quick/文件操作都经 pkexec（polkit 授权框）。
//! 密钥类数据一律通过 stdin 传递，绝不进入命令行参数（避免泄露到 /proc 进程列表）。

use std::io::Write;
use std::process::{Command, Output, Stdio};

/// 经 pkexec 执行一条命令。`stdin` 有值时写入子进程 stdin 后关闭（EOF）。
pub fn pkexec(args: &[&str], stdin: Option<&[u8]>) -> Result<Output, String> {
    let mut cmd = Command::new("/usr/bin/pkexec");
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("无法启动 pkexec（{}）: {e}", args.first().unwrap_or(&"")))?;

    if let Some(data) = stdin {
        child
            .stdin
            .take()
            .expect("stdin piped")
            .write_all(data)
            .map_err(|e| format!("写入 stdin 失败: {e}"))?;
    }

    let out = child
        .wait_with_output()
        .map_err(|e| format!("等待 pkexec 失败: {e}"))?;

    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let code = out.status.code().map(|c| c.to_string()).unwrap_or_else(|| "signal".into());
        return Err(if err.is_empty() {
            format!("命令失败（exit {code}）")
        } else {
            format!("命令失败（exit {code}）: {err}")
        });
    }
    Ok(out)
}

/// 特权执行 wg，返回 stdout 字节。
pub fn wg(args: &[&str], stdin: Option<&[u8]>) -> Result<Vec<u8>, String> {
    let mut full: Vec<&str> = vec!["/usr/bin/wg"];
    full.extend_from_slice(args);
    Ok(pkexec(&full, stdin)?.stdout)
}

/// 特权执行 wg-quick。
pub fn wg_quick(args: &[&str]) -> Result<(), String> {
    let mut full: Vec<&str> = vec!["/usr/bin/wg-quick"];
    full.extend_from_slice(args);
    pkexec(&full, None).map(|_| ())
}

/// 特权读取文件内容。
pub fn read_file(path: &str) -> Result<String, String> {
    let out = pkexec(&["/usr/bin/cat", path], None)?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// 特权写入文件（stdin 内容），可选设置权限。
pub fn write_file(path: &str, content: &str, mode: Option<&str>) -> Result<(), String> {
    pkexec(&["/usr/bin/tee", path], Some(content.as_bytes()))?;
    if let Some(m) = mode {
        pkexec(&["/usr/bin/chmod", m, path], None)?;
    }
    Ok(())
}
