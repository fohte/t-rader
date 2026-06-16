use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

/// Python サンドボックス実行の設定
#[derive(Debug, Clone)]
pub struct PythonSandboxConfig {
    pub nsjail_path: PathBuf,
    pub python_path: PathBuf,
    /// readonly bind mount で sandbox 内に見せるホスト側パス
    pub bind_mounts_ro: Vec<PathBuf>,
    /// nsjail の wall-clock 上限 (秒粒度)
    pub time_limit: Duration,
    /// stdout の最大バイト数。超過した場合は StdoutTooLarge を返す
    pub max_stdout_bytes: u64,
    /// stderr の最大バイト数。超過した場合は StderrTooLarge を返す
    pub max_stderr_bytes: u64,
    /// アドレス空間上限 (MB)。nsjail の --rlimit_as に渡す
    pub rlimit_as_mb: u64,
    /// ファイル書き込みサイズ上限 (MB)。nsjail の --rlimit_fsize に渡す
    pub rlimit_fsize_mb: u64,
    /// open file descriptor 上限
    pub rlimit_nofile: u64,
    /// child の最大プロセス数
    pub rlimit_nproc: u64,
}

impl Default for PythonSandboxConfig {
    fn default() -> Self {
        Self {
            nsjail_path: PathBuf::from("nsjail"),
            python_path: PathBuf::from("/usr/bin/python3"),
            bind_mounts_ro: vec![
                PathBuf::from("/usr"),
                PathBuf::from("/lib"),
                PathBuf::from("/lib64"),
                PathBuf::from("/bin"),
            ],
            time_limit: Duration::from_secs(5),
            max_stdout_bytes: 1024 * 1024,
            max_stderr_bytes: 256 * 1024,
            rlimit_as_mb: 512,
            rlimit_fsize_mb: 16,
            rlimit_nofile: 32,
            rlimit_nproc: 32,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PythonSandboxError {
    /// nsjail バイナリが見つからない。コンテナイメージに同梱されていない場合などに発生する
    #[error("nsjail binary not found: {0}")]
    NsjailNotFound(String),

    /// stdin に Python コードを書き込めなかった
    #[error("failed to write stdin: {0}")]
    WriteStdin(std::io::Error),

    /// 子プロセスの起動・wait で I/O エラー
    #[error("sandbox io error: {0}")]
    Io(#[from] std::io::Error),

    /// time_limit を超えても終了しなかった
    #[error("execution exceeded time limit ({0:?})")]
    Timeout(Duration),

    /// stdout が上限を超えた
    #[error("stdout exceeded {0} bytes")]
    StdoutTooLarge(u64),

    /// stderr が上限を超えた
    #[error("stderr exceeded {0} bytes")]
    StderrTooLarge(u64),
}

/// サンドボックス実行の結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PythonSandboxOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// 子プロセスの exit code。シグナルで終わった場合は None
    pub exit_code: Option<i32>,
}

/// nsjail 配下で Python コードを実行する。
///
/// `code` は stdin として渡され、`python3 - <args...>` で評価される。
/// network namespace は nsjail デフォルトで隔離され、ファイルシステムは
/// `config.bind_mounts_ro` の readonly bind mount と `/tmp` の tmpfs のみが見える。
pub async fn run_python(
    code: &str,
    args: &[String],
    config: &PythonSandboxConfig,
) -> Result<PythonSandboxOutput, PythonSandboxError> {
    let mut cmd = Command::new(&config.nsjail_path);
    cmd.arg("--mode")
        .arg("o")
        .arg("--quiet")
        .arg("--user")
        .arg("99999")
        .arg("--group")
        .arg("99999")
        .arg("--time_limit")
        .arg(config.time_limit.as_secs().to_string())
        .arg("--max_cpus")
        .arg("1")
        .arg("--rlimit_as")
        .arg(config.rlimit_as_mb.to_string())
        .arg("--rlimit_fsize")
        .arg(config.rlimit_fsize_mb.to_string())
        .arg("--rlimit_cpu")
        .arg(config.time_limit.as_secs().to_string())
        .arg("--rlimit_nofile")
        .arg(config.rlimit_nofile.to_string())
        .arg("--rlimit_nproc")
        .arg(config.rlimit_nproc.to_string())
        .arg("--disable_proc")
        .arg("--iface_no_lo")
        .arg("--cwd")
        .arg("/tmp")
        .arg("--tmpfsmount")
        .arg("/tmp")
        .arg("--seccomp_string")
        .arg(SECCOMP_POLICY);

    for path in &config.bind_mounts_ro {
        cmd.arg("--bindmount_ro").arg(path);
    }

    cmd.arg("--").arg(&config.python_path).arg("-");
    for a in args {
        cmd.arg(a);
    }

    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // run_python が早期 return / panic / cancel しても nsjail プロセスを残さない
        .kill_on_drop(true);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(PythonSandboxError::NsjailNotFound(
                config.nsjail_path.display().to_string(),
            ));
        }
        Err(e) => return Err(PythonSandboxError::Io(e)),
    };

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(code.as_bytes())
            .await
            .map_err(PythonSandboxError::WriteStdin)?;
        stdin
            .shutdown()
            .await
            .map_err(PythonSandboxError::WriteStdin)?;
    }

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let max_stdout = config.max_stdout_bytes;
    let max_stderr = config.max_stderr_bytes;

    let mut out_handle = tokio::spawn(async move { read_capped(stdout, max_stdout).await });
    let mut err_handle = tokio::spawn(async move { read_capped(stderr, max_stderr).await });

    let sleep = tokio::time::sleep(config.time_limit + Duration::from_secs(2));
    tokio::pin!(sleep);

    let mut status = None;
    let mut stdout_res: Option<CapResult> = None;
    let mut stderr_res: Option<CapResult> = None;

    // reader が cap 到達を検知した時点で即 kill する fail-fast loop。
    // 子プロセスが write を止めて長時間 sleep するケースで、time_limit いっぱい
    // 待たされる挙動を防ぐ
    while status.is_none() || stdout_res.is_none() || stderr_res.is_none() {
        tokio::select! {
            res = child.wait(), if status.is_none() => {
                status = Some(res.map_err(PythonSandboxError::Io)?);
            }
            res = &mut out_handle, if stdout_res.is_none() => {
                let val = res
                    .map_err(std::io::Error::other)
                    .map_err(PythonSandboxError::Io)?
                    .map_err(PythonSandboxError::Io)?;
                if matches!(val, CapResult::Exceeded(_)) {
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                    return Err(PythonSandboxError::StdoutTooLarge(max_stdout));
                }
                stdout_res = Some(val);
            }
            res = &mut err_handle, if stderr_res.is_none() => {
                let val = res
                    .map_err(std::io::Error::other)
                    .map_err(PythonSandboxError::Io)?
                    .map_err(PythonSandboxError::Io)?;
                if matches!(val, CapResult::Exceeded(_)) {
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                    return Err(PythonSandboxError::StderrTooLarge(max_stderr));
                }
                stderr_res = Some(val);
            }
            _ = &mut sleep => {
                let _ = child.start_kill();
                let _ = child.wait().await;
                return Err(PythonSandboxError::Timeout(config.time_limit));
            }
        }
    }

    Ok(PythonSandboxOutput {
        stdout: stdout_res.map(CapResult::into_bytes).unwrap_or_default(),
        stderr: stderr_res.map(CapResult::into_bytes).unwrap_or_default(),
        exit_code: status.and_then(|s| s.code()),
    })
}

// nsjail の kafel 文法で危険な syscall を KILL に倒す blocklist。
// 一般的なコンテナランタイム (Docker / podman) の default seccomp prof で blocked
// になっている syscall のうち、ユーザー名前空間と非 root ユーザー前提でも
// サンドボックス境界を破りうるもの (kernel attack surface 縮小・名前空間操作・
// カーネル keyring・eBPF・モジュール・kexec・ptrace 系) を列挙する。
const SECCOMP_POLICY: &str = "KILL { \
    ptrace, \
    process_vm_readv, \
    process_vm_writev, \
    bpf, \
    add_key, \
    request_key, \
    keyctl, \
    perf_event_open, \
    mount, \
    umount2, \
    pivot_root, \
    open_tree, \
    move_mount, \
    fsopen, \
    fsmount, \
    fsconfig, \
    fspick, \
    init_module, \
    finit_module, \
    delete_module, \
    setns, \
    unshare, \
    userfaultfd, \
    reboot, \
    kexec_load, \
    kexec_file_load, \
    swapon, \
    swapoff, \
    sysfs, \
    quotactl, \
    quotactl_fd, \
    nfsservctl, \
    lookup_dcookie, \
    open_by_handle_at, \
    name_to_handle_at, \
    ioperm, \
    iopl, \
    kcmp, \
    personality, \
    clock_settime, \
    clock_adjtime, \
    settimeofday, \
    adjtimex, \
    stime, \
    vhangup, \
    vm86, \
    vm86old, \
    modify_ldt \
} DEFAULT ALLOW";

enum CapResult {
    Ok(Vec<u8>),
    Exceeded(Vec<u8>),
}

impl CapResult {
    fn into_bytes(self) -> Vec<u8> {
        match self {
            CapResult::Ok(b) | CapResult::Exceeded(b) => b,
        }
    }
}

async fn read_capped<R: tokio::io::AsyncRead + Unpin>(
    reader: Option<R>,
    cap: u64,
) -> std::io::Result<CapResult> {
    let Some(reader) = reader else {
        return Ok(CapResult::Ok(Vec::new()));
    };
    let mut limited = reader.take(cap.saturating_add(1));
    let mut buf = Vec::new();
    limited.read_to_end(&mut buf).await?;
    if (buf.len() as u64) > cap {
        buf.truncate(cap as usize);
        Ok(CapResult::Exceeded(buf))
    } else {
        Ok(CapResult::Ok(buf))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use std::sync::OnceLock;

    /// CI 環境では nsjail 必須にして silent skip を防ぐ。それ以外 (macOS や nsjail
    /// 未導入のホスト) では skip する。skip するなら false を返す
    async fn require_nsjail_or_skip() -> bool {
        if nsjail_available().await {
            return true;
        }
        if std::env::var_os("CI").is_some() {
            panic!("nsjail is required when CI=true but was not found in PATH");
        }
        eprintln!("skipping: nsjail not available");
        false
    }

    /// nsjail が利用可能か (バイナリが存在し、`--help` が成功するか) を判定する。
    async fn nsjail_available() -> bool {
        static AVAILABLE: OnceLock<bool> = OnceLock::new();
        if let Some(v) = AVAILABLE.get() {
            return *v;
        }
        let ok = matches!(
            tokio::process::Command::new("nsjail")
                .arg("--help")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .stdin(Stdio::null())
                .status()
                .await,
            Ok(s) if s.code().is_some()
        );
        let _ = AVAILABLE.set(ok);
        ok
    }

    fn cfg() -> PythonSandboxConfig {
        PythonSandboxConfig::default()
    }

    #[tokio::test]
    async fn basic_arithmetic() {
        if !require_nsjail_or_skip().await {
            return;
        }
        let out = run_python("print(1 + 1)", &[], &cfg())
            .await
            .expect("run_python");
        assert_eq!(
            (
                String::from_utf8_lossy(&out.stdout).into_owned(),
                String::from_utf8_lossy(&out.stderr).into_owned(),
                out.exit_code,
            ),
            ("2\n".to_string(), String::new(), Some(0)),
        );
    }

    #[tokio::test]
    async fn stdlib_works_with_args() {
        if !require_nsjail_or_skip().await {
            return;
        }
        let code = indoc! {r#"
            import sys, json
            print(json.dumps({"args": sys.argv[1:], "sum": sum(range(10))}))
        "#};
        let out = run_python(code, &["a".to_string(), "b".to_string()], &cfg())
            .await
            .expect("run_python");
        assert_eq!(
            (
                out.exit_code,
                String::from_utf8_lossy(&out.stdout).into_owned()
            ),
            (
                Some(0),
                "{\"args\": [\"a\", \"b\"], \"sum\": 45}\n".to_string()
            ),
        );
    }

    #[tokio::test]
    async fn network_is_blocked() {
        if !require_nsjail_or_skip().await {
            return;
        }
        // 新規 netns には経路が無いので connect は失敗するはず
        let code = indoc! {r#"
            import socket
            try:
                socket.create_connection(("1.1.1.1", 80), timeout=1)
                print("CONNECTED")
            except OSError as e:
                print("BLOCKED")
        "#};
        let out = run_python(code, &[], &cfg()).await.expect("run_python");
        assert_eq!(
            (
                out.exit_code,
                String::from_utf8_lossy(&out.stdout).trim().to_string()
            ),
            (Some(0), "BLOCKED".to_string()),
        );
    }

    #[tokio::test]
    async fn host_files_are_hidden() {
        if !require_nsjail_or_skip().await {
            return;
        }
        // /etc はホストから bind mount していないのでアクセスできないはず
        let code = indoc! {r#"
            import os
            print("EXISTS" if os.path.exists("/etc/passwd") else "MISSING")
        "#};
        let out = run_python(code, &[], &cfg()).await.expect("run_python");
        assert_eq!(
            (
                out.exit_code,
                String::from_utf8_lossy(&out.stdout).trim().to_string()
            ),
            (Some(0), "MISSING".to_string()),
        );
    }

    #[tokio::test]
    async fn timeout_kills_runaway_loop() {
        if !require_nsjail_or_skip().await {
            return;
        }
        let mut config = cfg();
        config.time_limit = Duration::from_secs(1);
        let result = run_python("while True: pass", &[], &config).await;
        // nsjail の --time_limit が先に SIGKILL を送って exit_code=None で正常 return する
        // ケースと、外側 timeout が先に発火して Timeout が返るケースの両方が許容される
        match result {
            Ok(out) => assert_ne!(
                out.exit_code,
                Some(0),
                "timeout should not yield exit 0: {out:?}"
            ),
            Err(PythonSandboxError::Timeout(_)) => {}
            Err(other) => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[tokio::test]
    async fn stdout_size_limit_is_enforced() {
        if !require_nsjail_or_skip().await {
            return;
        }
        let mut config = cfg();
        config.max_stdout_bytes = 1024;
        // Linux の pipe buffer (default 64KB) を上回る量を書く。reader を drop した後も
        // Python 側が SIGPIPE で終了し、外側 timeout に救われずに StdoutTooLarge が返る
        // ことを保証する
        let code = indoc! {r#"
            import sys
            sys.stdout.write('x' * 256_000)
        "#};
        let err = run_python(code, &[], &config)
            .await
            .expect_err("should exceed stdout cap");
        assert_eq!(err.to_string(), "stdout exceeded 1024 bytes");
    }

    #[tokio::test]
    async fn nsjail_not_found_is_distinct() {
        let mut config = cfg();
        config.nsjail_path = PathBuf::from("/nonexistent/nsjail-binary");
        let err = run_python("print(0)", &[], &config)
            .await
            .expect_err("should fail to spawn");
        assert_eq!(
            err.to_string(),
            "nsjail binary not found: /nonexistent/nsjail-binary",
        );
    }
}
