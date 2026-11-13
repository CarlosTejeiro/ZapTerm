// Prevents an additional console window on Windows in release builds. Do not remove.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Linux/WebKitGTK launch-environment fixups.
    //
    // Two Wayland problems make the terminal unusable on some Linux setups
    // (both reproduced on Fedora + KDE Plasma + Wayland). Both must be fixed in
    // the environment *before* WebKit/GTK initializes — setting them in-process
    // is too late — so we re-exec ourselves once with the vars set. A dedicated
    // ZAPTERM_REEXEC sentinel on the child prevents an infinite re-exec loop
    // (we can't guard on the vars themselves because the session pre-sets some
    // of them, e.g. GTK_IM_MODULE=ibus, so their presence isn't a reliable
    // guard — we must override).
    //
    // 1. GDK backend. WebKitGTK's *native-Wayland* input handling duplicates
    //    keystrokes in the terminal ("crazy"/ghost characters, one input per
    //    keypress). This is independent of the NVIDIA driver — it reproduces
    //    both with the proprietary driver installed and without it — and it is
    //    NOT the IME issue below: it persists even with the IME modules emptied.
    //    Running under XWayland (GDK_BACKEND=x11) fixes it completely. XWayland
    //    is present in every Wayland session, so forcing x11 there is a safe
    //    default. We only touch this on a Wayland session (on native X11 it's
    //    already x11 and there's no bug), we respect a user-set GDK_BACKEND, and
    //    a user who wants native Wayland opts out with ZAPTERM_ENABLE_WAYLAND=1.
    //
    // 2. IME. WebKitGTK also mishandles the IBus/Fcitx input-method system on
    //    Wayland and garbles typing. Launching with GTK_IM_MODULE="" and
    //    QT_IM_MODULE="" (empty, i.e. direct keyboard input, no IME) fixes that
    //    part. Forced off by default; a user who needs composed/CJK/emoji input
    //    opts out with ZAPTERM_ENABLE_IME=1.
    //
    // 3. DMABUF renderer — off only inside an xrdp session. We do NOT force
    //    WEBKIT_DISABLE_DMABUF_RENDERER everywhere: it was a workaround for a
    //    busy-repaint/high-CPU issue on the NVIDIA *proprietary* driver, but
    //    forcing it off broke rendering on other setups (ARM, and x86_64 hybrid
    //    laptops on Intel/nouveau). An xrdp session is different: its X server
    //    has no GPU (no DRI3), so the DMABUF path composites every frame in
    //    software. Measured on an xrdp VM: WebKitWebProcess at 100% of a core
    //    with it, ~17% without. xrdp marks its sessions with XRDP_SESSION, so we
    //    key on that alone. Elsewhere it stays opt-in: a user hitting the
    //    high-CPU issue exports WEBKIT_DISABLE_DMABUF_RENDERER=1 themselves (it's
    //    inherited into the re-exec'd child), and a value they set — even "0" —
    //    is never overridden.
    //
    // Escape hatch: ZAPTERM_NO_LAUNCH_ENV=1 skips ALL of the above (no re-exec, no
    // env overrides), so a user can isolate whether one of these fixups is
    // causing a problem on their machine.
    #[cfg(target_os = "linux")]
    if std::env::var_os("ZAPTERM_REEXEC").is_none()
        && std::env::var_os("ZAPTERM_NO_LAUNCH_ENV").is_none()
    {
        // Collect the (name, value) env changes we need for this launch.
        let planned =
            launch_env_plan(|k| std::env::var_os(k).map(|v| v.to_string_lossy().into_owned()));

        if !planned.is_empty() {
            use std::os::unix::process::CommandExt;
            if let Ok(exe) = std::env::current_exe() {
                let mut cmd = std::process::Command::new(exe);
                cmd.args(std::env::args_os().skip(1));
                for (k, v) in &planned {
                    cmd.env(k, v);
                }
                // Sentinel so the re-exec'd child short-circuits above and
                // doesn't re-exec again (prevents an infinite loop).
                cmd.env("ZAPTERM_REEXEC", "1");
                // exec() replaces this process image and only returns on failure.
                let err = cmd.exec();
                // Re-exec failed (rare): set the vars in-process as a best-effort
                // fallback and carry on rather than abort startup. In-process may
                // be too late for WebKit/GTK (same caveat as always), but it's
                // better than nothing.
                eprintln!("zapterm: could not re-exec to set launch env ({err}); continuing");
                for (k, v) in &planned {
                    std::env::set_var(k, v);
                }
            }
        }
    }

    app::run();
}

/// The (name, value) env changes this launch needs, decided from the current
/// environment as seen through `var` (name → value if set). Pure, so the rules
/// above are unit-testable without touching the real process environment.
#[cfg(target_os = "linux")]
fn launch_env_plan(var: impl Fn(&str) -> Option<String>) -> Vec<(&'static str, &'static str)> {
    let mut planned = Vec::new();

    // GDK backend: force XWayland on a Wayland session unless the user set
    // GDK_BACKEND themselves or opted back into native Wayland.
    let on_wayland = var("WAYLAND_DISPLAY").is_some()
        || var("XDG_SESSION_TYPE").is_some_and(|v| v.eq_ignore_ascii_case("wayland"));
    if on_wayland && var("GDK_BACKEND").is_none() && var("ZAPTERM_ENABLE_WAYLAND").is_none() {
        planned.push(("GDK_BACKEND", "x11"));
    }

    // IME (all Linux arches): force direct keyboard input unless the user
    // opted back into their session IME via ZAPTERM_ENABLE_IME.
    if var("ZAPTERM_ENABLE_IME").is_none() {
        planned.push(("GTK_IM_MODULE", ""));
        planned.push(("QT_IM_MODULE", ""));
    }

    // DMABUF: off inside xrdp (software-only X server), unless the user set
    // the variable themselves.
    if var("XRDP_SESSION").is_some() && var("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        planned.push(("WEBKIT_DISABLE_DMABUF_RENDERER", "1"));
    }

    planned
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::launch_env_plan;

    fn plan(vars: &[(&str, &str)]) -> Vec<(&'static str, &'static str)> {
        launch_env_plan(|k| {
            vars.iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| v.to_string())
        })
    }

    const DMABUF: (&str, &str) = ("WEBKIT_DISABLE_DMABUF_RENDERER", "1");

    #[test]
    fn xrdp_session_disables_dmabuf() {
        // The variables an xrdp session on Ubuntu actually exports.
        let p = plan(&[
            ("XRDP_SESSION", "1"),
            ("XDG_SESSION_TYPE", "x11"),
            ("DISPLAY", ":10.0"),
        ]);
        assert!(p.contains(&DMABUF));
        assert!(
            !p.iter().any(|(k, _)| *k == "GDK_BACKEND"),
            "x11 session needs no backend override"
        );
    }

    #[test]
    fn local_sessions_keep_dmabuf() {
        assert!(!plan(&[("XDG_SESSION_TYPE", "x11"), ("DISPLAY", ":0")]).contains(&DMABUF));
        assert!(!plan(&[
            ("XDG_SESSION_TYPE", "wayland"),
            ("WAYLAND_DISPLAY", "wayland-0")
        ])
        .contains(&DMABUF));
    }

    #[test]
    fn user_set_dmabuf_value_is_never_overridden() {
        let p = plan(&[
            ("XRDP_SESSION", "1"),
            ("WEBKIT_DISABLE_DMABUF_RENDERER", "0"),
        ]);
        assert!(!p
            .iter()
            .any(|(k, _)| *k == "WEBKIT_DISABLE_DMABUF_RENDERER"));
    }

    #[test]
    fn existing_rules_unchanged() {
        let p = plan(&[("WAYLAND_DISPLAY", "wayland-0")]);
        assert!(p.contains(&("GDK_BACKEND", "x11")));
        assert!(p.contains(&("GTK_IM_MODULE", "")));
        assert!(
            !plan(&[("WAYLAND_DISPLAY", "w"), ("GDK_BACKEND", "wayland")])
                .contains(&("GDK_BACKEND", "x11"))
        );
        assert!(
            !plan(&[("WAYLAND_DISPLAY", "w"), ("ZAPTERM_ENABLE_WAYLAND", "1")])
                .contains(&("GDK_BACKEND", "x11"))
        );
        assert!(!plan(&[("ZAPTERM_ENABLE_IME", "1")])
            .iter()
            .any(|(k, _)| *k == "GTK_IM_MODULE"));
    }
}
