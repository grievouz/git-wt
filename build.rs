use std::process::Command;

fn main() {
    let date = Command::new("date")
        .args(["-u", "+%Y-%m-%d"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());

    println!("cargo:rustc-env=GIT_WT_BUILD_DATE={date}");
    println!("cargo:rerun-if-changed=build.rs");
}
