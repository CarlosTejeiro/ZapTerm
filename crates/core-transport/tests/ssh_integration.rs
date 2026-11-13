//! End-to-end tests of the SSH transport against a **real OpenSSH server**.
//!
//! They exercise the paths a unit test can't: host-key trust (preflight →
//! trust → changed → overwrite), every auth method (password, key files,
//! keyboard-interactive, agent), ProxyJump, local and remote port forwards
//! and SFTP — i.e. exactly the surface the russh upgrade touched.
//!
//! **Skipped unless `ZAPTERM_SSH_TEST_PORT` is set.** `scripts/ci/ssh-test-server.sh`
//! starts a throw-away sshd on 127.0.0.1 with a test user and prints the
//! variables; CI runs it before this test. Everything runs in ONE sequential
//! test function so `HOME` can be pointed at a scratch directory (the code
//! under test reads/writes `~/.ssh/known_hosts`) without racing other tests.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use core_transport::error::Error;
use core_transport::forwards::{open_local_forward, open_remote_forward};
use core_transport::sftp::{new_cancel_token, ProgressFn, SftpClient};
use core_transport::ssh::{
    self, AgentPriority, HostKeyStatus, KiRequest, KiResponder, SshAuth, SshTransport,
};
use core_transport::SessionCmd;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Everything the server script hands us. Only the port is mandatory; the
/// rest has defaults matching the script.
struct Env {
    host: String,
    port: u16,
    user: String,
    password: String,
    key_ed25519: PathBuf,
    key_rsa: PathBuf,
    key_enc: PathBuf,
    key_enc_passphrase: String,
    sftp_dir: String,
}

fn env() -> Option<Env> {
    let port = std::env::var("ZAPTERM_SSH_TEST_PORT").ok()?.parse().ok()?;
    let get = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
    Some(Env {
        host: get("ZAPTERM_SSH_TEST_HOST", "127.0.0.1"),
        port,
        user: get("ZAPTERM_SSH_TEST_USER", "zaptermtest"),
        password: get("ZAPTERM_SSH_TEST_PASSWORD", "Passw0rd!"),
        key_ed25519: PathBuf::from(get("ZAPTERM_SSH_TEST_KEY_ED25519", "")),
        key_rsa: PathBuf::from(get("ZAPTERM_SSH_TEST_KEY_RSA", "")),
        key_enc: PathBuf::from(get("ZAPTERM_SSH_TEST_KEY_ENC", "")),
        key_enc_passphrase: get("ZAPTERM_SSH_TEST_KEY_ENC_PASSPHRASE", "s3cret"),
        sftp_dir: get("ZAPTERM_SSH_TEST_SFTP_DIR", "/tmp"),
    })
}

const OP_TIMEOUT: Duration = Duration::from_secs(30);

/// Bound every network step so a regression can't hang the whole CI job.
async fn within<T>(label: &str, fut: impl Future<Output = T>) -> T {
    match tokio::time::timeout(OP_TIMEOUT, fut).await {
        Ok(v) => v,
        Err(_) => panic!("{label}: timed out after {OP_TIMEOUT:?}"),
    }
}

/// Open a PTY shell with `auth`, run a command whose output the server must
/// compute (so a real shell, not an echo, is proven), and return the captured
/// output once the marker shows up. Also hands back the session handle and
/// forward registry for the forward/SFTP scenarios.
async fn shell_marker(
    e: &Env,
    auth: SshAuth,
    label: &str,
) -> (
    tokio::sync::mpsc::UnboundedSender<SessionCmd>,
    core_transport::SharedSshHandle,
    ssh::RemoteForwardRegistry,
) {
    let transport = within(
        label,
        SshTransport::open_shell(e.host.clone(), e.port, e.user.clone(), auth, 80, 24),
    )
    .await
    .unwrap_or_else(|err| panic!("{label}: open_shell failed: {err}"));

    let out = Arc::new(Mutex::new(Vec::<u8>::new()));
    let sink = Arc::clone(&out);
    let (tx, handle, registry) =
        transport.start_io_loop(move |bytes| sink.lock().unwrap().extend(bytes), |_| {});
    tx.send(SessionCmd::Data(b"echo ZAPTERM_MARK_$((6*7))\n".to_vec()))
        .expect("send to shell");
    wait_for_output(&out, "ZAPTERM_MARK_42", label).await;
    (tx, handle, registry)
}

async fn wait_for_output(out: &Arc<Mutex<Vec<u8>>>, needle: &str, label: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        if String::from_utf8_lossy(&out.lock().unwrap()).contains(needle) {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "{label}: never saw {needle:?} in shell output:\n{}",
            String::from_utf8_lossy(&out.lock().unwrap())
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// A TCP echo server on 127.0.0.1 for the forward scenarios. Returns its port.
async fn spawn_echo_server() -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut s, _)) = listener.accept().await {
            tokio::spawn(async move {
                let (mut r, mut w) = s.split();
                let _ = tokio::io::copy(&mut r, &mut w).await;
            });
        }
    });
    port
}

/// Connect to `port`, send a line, expect it back verbatim through whatever
/// forward sits in between.
async fn echo_roundtrip(port: u16, payload: &str, label: &str) {
    let mut s = within(label, TcpStream::connect(("127.0.0.1", port)))
        .await
        .unwrap_or_else(|e| panic!("{label}: connect to forward port {port} failed: {e}"));
    s.write_all(payload.as_bytes()).await.unwrap();
    let mut buf = vec![0u8; payload.len()];
    within(label, s.read_exact(&mut buf))
        .await
        .unwrap_or_else(|e| panic!("{label}: read back failed: {e}"));
    assert_eq!(
        buf,
        payload.as_bytes(),
        "{label}: forward corrupted the bytes"
    );
}

fn password_responder(password: String) -> KiResponder {
    Arc::new(move |req: KiRequest| {
        let pw = password.clone();
        Box::pin(async move { vec![pw; req.prompts.len()] })
            as Pin<Box<dyn Future<Output = Vec<String>> + Send>>
    })
}

/// A running `ssh-agent` with the given key loaded; killed on drop.
struct TestAgent {
    child: std::process::Child,
    sock: PathBuf,
}

impl TestAgent {
    fn start(key: &Path, dir: &Path) -> Option<Self> {
        if key.as_os_str().is_empty() {
            return None;
        }
        let sock = dir.join("agent.sock");
        let child = std::process::Command::new("ssh-agent")
            .args(["-D", "-a"])
            .arg(&sock)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .ok()?;
        // The socket appears asynchronously.
        for _ in 0..100 {
            if sock.exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let added = std::process::Command::new("ssh-add")
            .arg(key)
            .env("SSH_AUTH_SOCK", &sock)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !added {
            return None;
        }
        Some(Self { child, sock })
    }
}

impl Drop for TestAgent {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = std::fs::remove_file(&self.sock);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ssh_end_to_end_against_real_openssh() {
    let Some(e) = env() else {
        eprintln!("ZAPTERM_SSH_TEST_PORT not set — skipping SSH integration test");
        return;
    };

    // Point HOME at a scratch dir: the transport reads and writes
    // ~/.ssh/known_hosts, and we must never touch the real one.
    let home = std::env::temp_dir().join(format!("zapterm-ssh-it-{}", std::process::id()));
    let ssh_dir = home.join(".ssh");
    std::fs::create_dir_all(&ssh_dir).unwrap();
    std::env::set_var("HOME", &home);
    let known_hosts = ssh_dir.join("known_hosts");

    // ── Host key: preflight → trust → known ────────────────────────────────
    let status = within("preflight", ssh::preflight_host_key(e.host.clone(), e.port))
        .await
        .expect("preflight");
    let fp = match status {
        HostKeyStatus::Unknown { fingerprint } => fingerprint,
        other => panic!("fresh known_hosts must report Unknown, got {other:?}"),
    };
    assert!(
        fp.starts_with("SHA256:"),
        "fingerprint format changed: {fp}"
    );
    within(
        "trust",
        ssh::trust_host_key(e.host.clone(), e.port, fp.clone()),
    )
    .await
    .expect("trust_host_key");
    assert!(matches!(
        ssh::preflight_host_key(e.host.clone(), e.port)
            .await
            .unwrap(),
        HostKeyStatus::Known
    ));

    // ── Host key: changed → overwrite → known ──────────────────────────────
    // Replace the recorded key with a different (but valid) ed25519 key so the
    // server now looks like a MITM, then take the one-click overwrite path.
    let other_pub = std::fs::read_to_string(e.key_ed25519.with_extension("pub"))
        .expect("read ed25519 .pub for the bogus entry");
    let mut parts = other_pub.split_whitespace();
    let (alg, blob) = (parts.next().unwrap(), parts.next().unwrap());
    let entry_host = if e.port == 22 {
        e.host.clone()
    } else {
        format!("[{}]:{}", e.host, e.port)
    };
    std::fs::write(&known_hosts, format!("{entry_host} {alg} {blob}\n")).unwrap();
    let changed = ssh::preflight_host_key(e.host.clone(), e.port)
        .await
        .unwrap();
    let HostKeyStatus::Changed { fingerprint: fp2 } = changed else {
        panic!("tampered known_hosts must report Changed, got {changed:?}");
    };
    assert_eq!(fp2, fp, "the server's real fingerprint must be reported");
    within(
        "overwrite",
        ssh::overwrite_host_key(e.host.clone(), e.port, fp2),
    )
    .await
    .expect("overwrite_host_key");
    assert!(matches!(
        ssh::preflight_host_key(e.host.clone(), e.port)
            .await
            .unwrap(),
        HostKeyStatus::Known
    ));

    // ── Password ───────────────────────────────────────────────────────────
    let (tx, handle, registry) =
        shell_marker(&e, SshAuth::Password(e.password.clone()), "password").await;

    let wrong = within(
        "wrong password",
        SshTransport::open_shell(
            e.host.clone(),
            e.port,
            e.user.clone(),
            SshAuth::Password("definitely-not-it".into()),
            80,
            24,
        ),
    )
    .await;
    assert!(
        matches!(wrong, Err(Error::AuthFailed)),
        "wrong password must be AuthFailed, got {:?}",
        wrong.err()
    );

    // ── Forwards on the password session ───────────────────────────────────
    let echo_port = spawn_echo_server().await;

    // -R: the server binds a port and hands us `forwarded-tcpip` channels —
    // the path that now has to answer the channel-open explicitly.
    let remote = within(
        "open -R",
        open_remote_forward(
            Arc::clone(&handle),
            Arc::clone(&registry),
            "127.0.0.1".into(),
            0,
            "127.0.0.1".into(),
            echo_port,
        ),
    )
    .await
    .expect("open_remote_forward");
    assert_ne!(
        remote.info.bind_port, 0,
        "server must report the port it bound"
    );
    echo_roundtrip(remote.info.bind_port, "ping over -R\n", "remote forward").await;
    drop(remote);

    // -L: our listener, `direct-tcpip` to the echo server through the session.
    let local = within(
        "open -L",
        open_local_forward(
            Arc::clone(&handle),
            "127.0.0.1".into(),
            0,
            "127.0.0.1".into(),
            echo_port,
        ),
    )
    .await
    .expect("open_local_forward");
    echo_roundtrip(local.info.bind_port, "ping over -L\n", "local forward").await;
    drop(local);

    // ── Shell lifecycle: long paste, then `exit` ───────────────────────────
    {
        let transport = within(
            "open lifecycle shell",
            SshTransport::open_shell(
                e.host.clone(),
                e.port,
                e.user.clone(),
                SshAuth::Password(e.password.clone()),
                80,
                24,
            ),
        )
        .await
        .expect("open_shell");
        let out = Arc::new(Mutex::new(Vec::<u8>::new()));
        let sink = Arc::clone(&out);
        let (closed_tx, closed_rx) = tokio::sync::oneshot::channel();
        let (tx, _h, _r) = transport.start_io_loop(
            move |b| sink.lock().unwrap().extend(b),
            move |reason| {
                let _ = closed_tx.send(reason);
            },
        );
        // A long paste the shell echoes back: writing must not stall reading
        // (writer and reader run separately).
        let mut paste = Vec::new();
        for i in 0..2000 {
            paste.extend_from_slice(format!(": line {i} of a long pasted config\n").as_bytes());
        }
        paste.extend_from_slice(b"echo PASTE_$((40+2))_DONE\n");
        tx.send(SessionCmd::Data(paste)).unwrap();
        wait_for_output(&out, "PASTE_42_DONE", "long paste").await;
        // `exit` ends the session cleanly: reported as Ended (no reconnect).
        tx.send(SessionCmd::Data(b"exit\n".to_vec())).unwrap();
        let reason = tokio::time::timeout(Duration::from_secs(10), closed_rx)
            .await
            .expect("on_close not called after exit")
            .unwrap();
        assert_eq!(reason, core_transport::CloseReason::Ended);
    }

    // ── SFTP on the password session ───────────────────────────────────────
    {
        let guard = handle.read().await;
        let sftp = within("sftp open", SftpClient::open(&guard))
            .await
            .expect("SftpClient::open");
        drop(guard);
        let payload: Vec<u8> = (0..1_000_003u32)
            .map(|i| (i.wrapping_mul(2654435761) >> 13) as u8)
            .collect();
        let local_up = home.join("upload.bin");
        std::fs::write(&local_up, &payload).unwrap();
        let remote_path = format!("{}/zapterm-it-{}.bin", e.sftp_dir, std::process::id());
        let sent = within("sftp upload", sftp.upload(&local_up, &remote_path))
            .await
            .expect("upload");
        assert_eq!(sent as usize, payload.len());
        let st = sftp.stat(&remote_path).await.expect("stat");
        assert_eq!(
            st.size,
            Some(payload.len() as u64),
            "remote size after upload"
        );
        let local_down = home.join("download.bin");
        let got = within("sftp download", sftp.download(&remote_path, &local_down))
            .await
            .expect("download");
        assert_eq!(got as usize, payload.len());
        assert_eq!(
            std::fs::read(&local_down).unwrap(),
            payload,
            "SFTP round-trip bytes"
        );
        // The variants the UI actually uses: streaming with progress + cancel.
        let remote_stream = format!(
            "{}/zapterm-it-{}-stream.bin",
            e.sftp_dir,
            std::process::id()
        );
        let progress_max = Arc::new(Mutex::new((0u64, None::<u64>)));
        let pm = Arc::clone(&progress_max);
        let progress: ProgressFn = Arc::new(move |done, total| {
            let mut g = pm.lock().unwrap();
            if done >= g.0 {
                *g = (done, total);
            }
        });
        let sent = within(
            "sftp upload_streaming",
            sftp.upload_streaming(
                &local_up,
                &remote_stream,
                new_cancel_token(),
                Arc::clone(&progress),
            ),
        )
        .await
        .expect("upload_streaming");
        assert_eq!(sent as usize, payload.len());
        assert_eq!(
            *progress_max.lock().unwrap(),
            (payload.len() as u64, Some(payload.len() as u64)),
            "upload progress must end at the total"
        );
        let local_stream = home.join("download-stream.bin");
        let got = within(
            "sftp download_streaming",
            sftp.download_streaming(&remote_stream, &local_stream, new_cancel_token(), progress),
        )
        .await
        .expect("download_streaming");
        assert_eq!(got as usize, payload.len());
        assert_eq!(
            std::fs::read(&local_stream).unwrap(),
            payload,
            "streaming SFTP round-trip bytes"
        );
        // A cancelled download over an existing local file leaves it as it
        // was (it used to be truncated, then deleted) and no partial file.
        let keep = home.join("keep.txt");
        std::fs::write(&keep, b"precious").unwrap();
        let cancelled = new_cancel_token();
        cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
        let res = within(
            "sftp download_streaming (cancelled)",
            sftp.download_streaming(&remote_stream, &keep, cancelled, Arc::new(|_, _| {})),
        )
        .await;
        assert!(res.is_err(), "a cancelled download must fail");
        assert_eq!(std::fs::read(&keep).unwrap(), b"precious");
        assert!(
            !std::fs::read_dir(&home).unwrap().any(|e| e
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".zapterm-part-")),
            "no partial file may be left behind"
        );
        sftp.remove_file(&remote_stream)
            .await
            .expect("remove_file (stream)");
        sftp.remove_file(&remote_path).await.expect("remove_file");
        assert!(
            !sftp
                .list_dir(&e.sftp_dir)
                .await
                .unwrap()
                .iter()
                .any(|x| remote_path.ends_with(&x.name)),
            "file must be gone after remove_file"
        );
    }
    drop(tx);

    // ── Key files: ed25519, RSA (rsa-sha2 negotiation), encrypted ──────────
    if !e.key_ed25519.as_os_str().is_empty() {
        let auth = SshAuth::PublicKey {
            key_path: e.key_ed25519.to_string_lossy().into_owned(),
            passphrase: None,
        };
        drop(shell_marker(&e, auth, "key ed25519").await);
    }
    if !e.key_rsa.as_os_str().is_empty() {
        let auth = SshAuth::PublicKey {
            key_path: e.key_rsa.to_string_lossy().into_owned(),
            passphrase: None,
        };
        drop(shell_marker(&e, auth, "key rsa").await);
    }
    if !e.key_enc.as_os_str().is_empty() {
        let path = e.key_enc.to_string_lossy().into_owned();
        drop(
            shell_marker(
                &e,
                SshAuth::PublicKey {
                    key_path: path.clone(),
                    passphrase: Some(e.key_enc_passphrase.clone()),
                },
                "encrypted key",
            )
            .await,
        );
        for (label, passphrase) in [
            ("no passphrase", None),
            ("wrong passphrase", Some("nope".to_string())),
        ] {
            let r = SshTransport::open_shell(
                e.host.clone(),
                e.port,
                e.user.clone(),
                SshAuth::PublicKey {
                    key_path: path.clone(),
                    passphrase,
                },
                80,
                24,
            )
            .await;
            assert!(
                matches!(r, Err(Error::KeyPassphrase)),
                "encrypted key with {label} must be KeyPassphrase, got {:?}",
                r.err()
            );
        }
    }

    // ── Keyboard-interactive (PAM password prompt) ─────────────────────────
    let ki = SshAuth::KeyboardInteractive {
        responder: password_responder(e.password.clone()),
    };
    drop(shell_marker(&e, ki, "keyboard-interactive").await);

    // ── Agent ──────────────────────────────────────────────────────────────
    match TestAgent::start(&e.key_ed25519, &home) {
        Some(agent) => {
            std::env::set_var("SSH_AUTH_SOCK", &agent.sock);
            drop(
                shell_marker(
                    &e,
                    SshAuth::Agent {
                        priority: AgentPriority::Auto,
                    },
                    "agent",
                )
                .await,
            );
            std::env::remove_var("SSH_AUTH_SOCK");
        }
        None => eprintln!("ssh-agent/ssh-add unavailable — agent scenario skipped"),
    }

    // ── ProxyJump: shell to the target THROUGH an authenticated bastion ────
    let bastion = within(
        "bastion",
        ssh::connect_authenticated(
            e.host.clone(),
            e.port,
            e.user.clone(),
            SshAuth::Password(e.password.clone()),
        ),
    )
    .await
    .expect("connect_authenticated");
    let via = within(
        "shell via jump",
        SshTransport::open_shell_via(
            vec![bastion],
            e.host.clone(),
            e.port,
            e.user.clone(),
            SshAuth::Password(e.password.clone()),
            80,
            24,
        ),
    )
    .await
    .expect("open_shell_via");
    let out = Arc::new(Mutex::new(Vec::<u8>::new()));
    let sink = Arc::clone(&out);
    let (tx, _h, _r) = via.start_io_loop(move |b| sink.lock().unwrap().extend(b), |_| {});
    tx.send(SessionCmd::Data(b"echo ZAPTERM_MARK_$((6*7))\n".to_vec()))
        .unwrap();
    wait_for_output(&out, "ZAPTERM_MARK_42", "jump").await;
    drop(tx);

    let _ = std::fs::remove_dir_all(&home);
}
