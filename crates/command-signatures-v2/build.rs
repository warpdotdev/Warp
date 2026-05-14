use command::blocking::Command;
use std::path::Path;

use anyhow::anyhow;

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=js/src");
    println!("cargo:rerun-if-changed=js/build");
    println!("cargo:rerun-if-changed=js/package.json");
    println!("cargo:rerun-if-changed=js/tsconfig.json");

    if let Err(e) = build_command_signatures() {
        if !Path::new(format!("{}/js/build", env!("CARGO_MANIFEST_DIR")).as_str()).exists() {
            panic!(
                r#"Failed to build command signatures JS: {e:?}.

Most likely, this is fixed by:
    1) Ensuring you have an up-to-date Node version; 18.14.1 (required for warp-server development) should suffice.
    2) Running `corepack enable` (this can be done in any directory).
    3) Removing a conflicting yarn installed by brew by running:
        brew uninstall yarn

If you continue to encounter issues, ensure you don't have conflicting Node installations, one of which might not be a sufficiently recent version.
"#
            )
        } else {
            println!("cargo:warning=Failed to build command signatures JS: {e:?}. Proceeding with stale command signatures!");
        }
    }
    Ok(())
}

fn build_command_signatures() -> anyhow::Result<()> {
    let mut command = if cfg!(windows) {
        let mut command = Command::new("cmd.exe");
        #[cfg(windows)]
        command.creation_flags(0);
        command.arg("/C").arg("yarn").arg("build");
        command
    } else {
        let mut command = Command::new("yarn");
        command.arg("build");
        command
    };

    match command
        .current_dir(format!("{}/js", env!("CARGO_MANIFEST_DIR")))
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                Ok(())
            } else {
                Err(anyhow!(
                    "Failed to build Command Signatures JS with output: {:?}",
                    output
                ))
            }
        }
        Err(e) => Err(anyhow::Error::from(e)),
    }
}
