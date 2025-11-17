// build.rs
use std::env;
use std::path::PathBuf;
use embed_resource::CompilationResult;
use fs_extra::dir::{copy, CopyOptions};

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let project_dir = env::var("CARGO_MANIFEST_DIR").unwrap(); // Project root
    let profile = env::var("PROFILE").unwrap(); // 'debug' or 'release'

    // --- Compile Resources (Windows Only) ---
    if target_os == "windows" {
        println!("cargo:rerun-if-changed=tray-icons.rc"); // Rerun if rc file changes
        // Handle the result of the resource compilation
        match embed_resource::compile("tray-icons.rc", embed_resource::NONE) {
            CompilationResult::Ok => {
                println!("Successfully compiled resources.");
            },
            CompilationResult::Failed(err) => {
                eprintln!("Error compiling resources: {}", err);
                std::process::exit(1); // Exit with error code
            },
            CompilationResult::NotAttempted(e) => {
                eprintln!("Resource compilation not attempted or not supported: {}", e);
                std::process::exit(1); // Exit with error code
            },
            _ => {
                eprintln!("Unknown error during resource compilation.");
                std::process::exit(1); // Exit with error code
            }
        }
    }

    // --- Copy Modules Directory ---
    let src_modules_path = PathBuf::from(&project_dir).join("modules");
    // The final executable is typically in target/{profile}/deps, but the user runs target/{profile}/executable_name
    // So we copy modules to target/{profile}/modules
    let target_dir = PathBuf::from(&project_dir).join("target").join(profile);
    let dest_modules_path = target_dir.join("modules");

    if src_modules_path.exists() {
        println!("cargo:rerun-if-changed=modules"); // Rerun if modules content changes
        let mut options = CopyOptions::new();
        options.overwrite = true; // Overwrite existing files in destination
        options.copy_inside = false; // Copy the 'modules' folder itself, not just its content

        match copy(&src_modules_path, &target_dir, &options) {
             Ok(_) => println!("Successfully copied modules to {}", dest_modules_path.display()),
             Err(e) => eprintln!("Error copying modules directory: {}", e), // Use eprintln for build script errors
        }
    } else {
        println!("Skipping module copy: source directory '{}' does not exist.", src_modules_path.display());
    }
}
