use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, ensure};

fn main() -> anyhow::Result<()> {
    // Compile all the base Slang shaders into a module.
    println!("cargo::rerun-if-changed=src");
    let out_dir: String = std::env::var("OUT_DIR").unwrap();
    let slang_paths: Vec<PathBuf> = {
        let mut paths = Vec::new();
        get_all_slang_file_paths(&mut paths, "src")
            .context("Error while finding '.slang' files")?;
        paths
    };
    for path in slang_paths {
        // Horribly ugly code to turn a file path like "src/chunk/block_face.slang" into
        // "chunk-block_face.spv".
        let module_file_name: String = {
            let mut out = path
                // Iterate over path components.
                .iter()
                // Skip over the initial "src" component.
                .skip(1)
                .map(|c| c.to_str().unwrap())
                .collect::<Vec<&str>>()
                .join("-");
            // Replace ending ".slang" with ".spv".
            out = out.strip_suffix(".slang").unwrap().to_string();
            out.push_str(".spv");
            out
        };
        let compile_status = Command::new("slangc")
            .args(["-profile", "spirv_1_5"])
            .arg("-fvk-use-entrypoint-name")
            .args(["-o", &format!("{out_dir}/{module_file_name}")])
            .arg("--")
            .arg(&path)
            .status()
            .with_context(|| format!("Failed to execute 'slangc' for {}", path.display()))?;
        ensure!(
            compile_status.success(),
            "'slangc' failed for {}, error code {}",
            path.display(),
            compile_status,
        );
    }
    Ok(())
}

fn get_all_slang_file_paths(out: &mut Vec<PathBuf>, path: impl AsRef<Path>) -> anyhow::Result<()> {
    for entry_result in std::fs::read_dir(path.as_ref()).context("Error while reading directory")? {
        let Ok(entry) = entry_result else {
            continue;
        };
        let Ok(entry_type) = entry.file_type() else {
            continue;
        };
        if entry_type.is_file() {
            let file_path = entry.path();
            let Ok(file_name) = entry.file_name().into_string() else {
                continue;
            };
            if !file_name.starts_with("_") && file_path.extension() == Some(OsStr::new("slang")) {
                out.push(file_path);
            }
        } else if entry_type.is_dir() {
            let dir_path = entry.path();
            _ = get_all_slang_file_paths(out, dir_path);
        }
    }
    Ok(())
}
