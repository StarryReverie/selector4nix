use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let workspace_root = Path::new(&manifest_dir).join("../..");

    for p in [
        "package.json",
        "package-lock.json",
        "vite.config.js",
        "frontend/src",
        "frontend/templates",
    ] {
        let p = workspace_root.join(p);
        println!("cargo:rerun-if-changed={}", p.display());
    }

    // Try to build the frontend if `npm` is available. Skip building and assume that the build
    // artifacts are already generated otherwise, typically when built with Nix.
    let is_npm_available = Command::new("npm")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if is_npm_available {
        // Fetch npm dependencies if not exist.
        if !workspace_root.join("node_modules").exists() {
            let status = Command::new("npm")
                .args(["clean-install"])
                .current_dir(&workspace_root)
                .status()
                .expect("failed to spawn `npm`");
            if !status.success() {
                panic!("failed to run `npm clean-install`");
            }
        }

        // Run frontend build.
        let status = Command::new("npm")
            .args(["run", "build"])
            .current_dir(&workspace_root)
            .status()
            .expect("failed to spawn `npm`");
        if !status.success() {
            panic!("failed to `npm run build`");
        }
    }

    // Always check whether the frontend artifacts exist and fail early if not.
    if !workspace_root.join("frontend/dist").exists() {
        panic!(
            "`frontend/dist/` not found. Install Node.js to build, or ensure the Nix derivation placed artifacts here."
        );
    }
    if !workspace_root.join("frontend/templates").exists() {
        panic!("`frontend/templates/` not found");
    }
}
