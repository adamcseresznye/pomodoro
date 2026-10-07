fn main() {
    println!("cargo:rerun-if-changed=assets/app-icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // Also support ordinary shells where the SDK isn't on PATH.
        if std::env::var_os("RC_PATH").is_none() {
            if let Some(program_files) = std::env::var_os("ProgramFiles(x86)") {
                let sdk = std::path::PathBuf::from(program_files).join("Windows Kits/10/bin");
                if let Ok(versions) = std::fs::read_dir(sdk) {
                    let mut compilers: Vec<_> = versions
                        .filter_map(Result::ok)
                        .map(|entry| entry.path().join("x64/rc.exe"))
                        .filter(|path| path.is_file())
                        .collect();
                    compilers.sort();
                    if let Some(compiler) = compilers.last() {
                        std::env::set_var("RC_PATH", compiler);
                    }
                }
            }
        }
        winresource::WindowsResource::new()
            .set_icon("assets/app-icon.ico")
            .set("ProductName", "Pomodoro Soundscapes")
            .set("FileDescription", "Pomodoro Soundscapes")
            .compile()
            .expect("could not embed the Windows application icon");
    }
}
