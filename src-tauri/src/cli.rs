//! Mole CLI 桥接。
//!
//! 破坏性操作（清理、卸载）不在本应用内实现，而是调用 `mo` 可执行文件，
//! 由 Mole 自己的安全层完成路径校验、废纸篓路由与操作日志记录。
//! 这样 GUI 不会绕过 `validate_path_for_deletion` 与 `mole_delete`。

use serde::Serialize;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

/// `mo` 的定位结果。
#[derive(Debug, Clone, Serialize)]
pub struct CliLocation {
    pub found: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    /// 未找到时给出的安装指引。
    pub install_hint: Option<String>,
}

/// 子进程执行结果。
#[derive(Debug, Clone, Serialize)]
pub struct CommandOutput {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// 候选可执行文件路径。
///
/// 顺序有意从「用户显式安装」到「包管理器前缀」：
/// 先查 PATH，再查 Homebrew 两个前缀，最后查手动安装目录。
fn candidate_paths() -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(path_var) = std::env::var("PATH") {
        for dir in path_var.split(':') {
            if dir.is_empty() {
                continue;
            }
            candidates.push(PathBuf::from(dir).join("mo"));
            candidates.push(PathBuf::from(dir).join("mole"));
        }
    }

    for prefix in [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/opt/mole",
    ] {
        candidates.push(PathBuf::from(prefix).join("mo"));
        candidates.push(PathBuf::from(prefix).join("mole"));
    }

    // 用户手动安装的常见位置。
    if let Some(home) = crate::safety::home_dir() {
        candidates.push(PathBuf::from(&home).join(".local/bin/mo"));
        candidates.push(PathBuf::from(&home).join(".local/bin/mole"));
    }

    candidates
}

/// 判断候选路径是否是可执行文件。
fn is_executable(path: &PathBuf) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    meta.is_file() && meta.permissions().mode() & 0o111 != 0
}

/// 定位 `mo` 并读取版本。
pub async fn locate() -> CliLocation {
    let Some(path) = candidate_paths().into_iter().find(is_executable) else {
        return CliLocation {
            found: false,
            path: None,
            version: None,
            install_hint: Some(
                "没有找到 Mole CLI。请先安装：brew install mole，\
                 或运行 curl -fsSL https://raw.githubusercontent.com/tw93/mole/main/install.sh | bash"
                    .to_string(),
            ),
        };
    };

    let version = match run_raw(&path, &["--version"]).await {
        Ok(out) if out.success => Some(
            out.stdout
                .lines()
                .find(|line| line.contains("version"))
                .unwrap_or(out.stdout.lines().next().unwrap_or(""))
                .trim()
                .to_string(),
        ),
        _ => None,
    };

    CliLocation {
        found: true,
        path: Some(path.to_string_lossy().into_owned()),
        version,
        install_hint: None,
    }
}

/// 解析出的 CLI 路径，供执行类命令复用。
pub async fn resolve_cli() -> Result<PathBuf, String> {
    let location = locate().await;
    match location.path {
        Some(p) => Ok(PathBuf::from(p)),
        None => Err(location
            .install_hint
            .unwrap_or_else(|| "没有找到 Mole CLI".to_string())),
    }
}

/// 执行 `mo` 子命令并等待结束。
async fn run_raw(program: &PathBuf, args: &[&str]) -> Result<CommandOutput, String> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // 关闭 stdin，避免 CLI 进入交互式菜单等待输入。
        .stdin(Stdio::null())
        .spawn()
        .map_err(|e| format!("启动 Mole 失败：{e}"))?;

    let mut stdout_buf = Vec::new();
    let mut stderr_buf = Vec::new();

    if let Some(mut out) = child.stdout.take() {
        out.read_to_end(&mut stdout_buf)
            .await
            .map_err(|e| format!("读取输出失败：{e}"))?;
    }
    if let Some(mut err) = child.stderr.take() {
        err.read_to_end(&mut stderr_buf)
            .await
            .map_err(|e| format!("读取错误输出失败：{e}"))?;
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("等待 Mole 结束失败：{e}"))?;

    Ok(CommandOutput {
        success: status.success(),
        exit_code: status.code(),
        stdout: String::from_utf8_lossy(&stdout_buf).into_owned(),
        stderr: String::from_utf8_lossy(&stderr_buf).into_owned(),
    })
}

/// 运行指定的 `mo` 子命令。
pub async fn run(args: &[&str]) -> Result<CommandOutput, String> {
    let program = resolve_cli().await?;
    run_raw(&program, args).await
}

/// 执行清理。`dry_run` 为 true 时只预览。
///
/// `MOLE_DRY_RUN=1` 是 Mole 官方支持的预览开关；这里额外传 `--dry-run`
/// 让两个入口一致，避免任一未被识别时行为不同。
pub async fn clean(dry_run: bool) -> Result<CommandOutput, String> {
    let mut args: Vec<&str> = vec!["clean"];
    if dry_run {
        args.push("--dry-run");
    }
    run(&args).await
}

/// 卸载指定应用。
///
/// 只接受应用名（Mole 接受的名字），不接受任意路径，避免把 GUI 变成
/// 一个可以删任何东西的入口。
pub async fn uninstall(app_name: &str, dry_run: bool) -> Result<CommandOutput, String> {
    if app_name.trim().is_empty() {
        return Err("应用名不能为空".to_string());
    }
    if app_name.contains('\n') || app_name.contains('\0') {
        return Err("应用名包含非法字符".to_string());
    }

    let mut args: Vec<&str> = vec!["uninstall"];
    if dry_run {
        args.push("--dry-run");
    }
    args.push(app_name);
    run(&args).await
}

/// 运行系统优化。Mole 的优化项都是有界维护任务。
pub async fn optimize(dry_run: bool) -> Result<CommandOutput, String> {
    let mut args: Vec<&str> = vec!["optimize"];
    if dry_run {
        args.push("--dry-run");
    }
    run(&args).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_include_homebrew_prefixes() {
        let candidates = candidate_paths();
        let as_strings: Vec<String> = candidates
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert!(as_strings.iter().any(|p| p == "/opt/homebrew/bin/mo"));
        assert!(as_strings.iter().any(|p| p == "/usr/local/bin/mole"));
    }

    #[test]
    fn rejects_empty_app_name() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        assert!(rt.block_on(uninstall("", false)).is_err());
        assert!(rt.block_on(uninstall("   ", true)).is_err());
    }

    #[test]
    fn rejects_control_characters_in_app_name() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        assert!(rt.block_on(uninstall("bad\nname", false)).is_err());
        assert!(rt.block_on(uninstall("bad\0name", false)).is_err());
    }

    #[test]
    fn locate_reports_missing_cli_with_hint() {
        // 本机未安装 Mole 时，必须给出安装提示而不是静默失败。
        let rt = tokio::runtime::Runtime::new().unwrap();
        let location = rt.block_on(locate());
        if !location.found {
            assert!(location.install_hint.is_some());
            assert!(location.path.is_none());
        }
    }

    #[test]
    fn is_executable_rejects_directories() {
        assert!(!is_executable(&PathBuf::from("/tmp")));
        assert!(!is_executable(&PathBuf::from("/definitely/not/here")));
    }
}
