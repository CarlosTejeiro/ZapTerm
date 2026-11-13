pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let release = args.iter().any(|a| a == "--release");
    let mut cmd = std::process::Command::new("cargo");
    cmd.args(["build", "--workspace"]);
    if release {
        cmd.arg("--release");
    }
    let status = cmd.status()?;
    if !status.success() {
        return Err("cargo build failed".into());
    }
    Ok(())
}
