fn main() {
    // The approved FFmpeg hashes are compiled into the app (see video_trim::BUILD_PINS).
    println!("cargo:rerun-if-env-changed=OPTIC_RIFT_FFMPEG_SHA256");
    println!("cargo:rerun-if-env-changed=OPTIC_RIFT_FFPROBE_SHA256");
    tauri_build::build();
    #[cfg(windows)]
    embed_test_manifest();
}

/// tauri-build embeds the Common Controls v6 manifest only into binaries. Test executables
/// that link Tauri then fail to start on Windows with STATUS_ENTRYPOINT_NOT_FOUND
/// (TaskDialogIndirect is missing from comctl32 v5). Give test targets the same dependency.
#[cfg(windows)]
fn embed_test_manifest() {
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let manifest = out_dir.join("test-app.manifest");
    std::fs::write(
        &manifest,
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*" />
    </dependentAssembly>
  </dependency>
</assembly>
"#,
    )
    .expect("write test manifest");
    println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-tests=/MANIFESTINPUT:{}",
        manifest.display()
    );
}
