use std::path::Path;
use std::process::Command;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let project_root = Path::new(&manifest_dir).parent().unwrap();
    let index_html = project_root.join("build").join("index.html");

    // Rerun this script if build/index.html disappears
    println!("cargo:rerun-if-changed={}", index_html.display());

    if !index_html.exists() {
        println!("cargo:warning=Frontend build missing — running `npm run build`");
        let status = Command::new("npm")
            .args(["run", "build"])
            .current_dir(project_root)
            .status()
            .expect("Failed to execute npm run build — is Node.js installed?");
        assert!(status.success(), "npm run build failed — check frontend for errors");
    }

    tauri_build::build()
}
