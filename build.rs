extern crate pkg_config;

#[cfg(feature = "bundled")]
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

#[cfg(not(feature = "bundled"))]
fn main() {
    let probed = pkg_config::Config::new().atleast_version("2.0.0").probe("hwloc");
    if probed.is_ok() {
        return;
    }
}

#[cfg(feature = "bundled")]
fn main() {
    setup_bundled_hwloc("2.8.0");
    find_hwloc(Some("2.8.0"));
}

#[cfg(feature = "bundled")]
fn setup_bundled_hwloc(required_version: &str) {
    // Determine which version to fetch and where to fetch it
    let source_version = match required_version
        .split('.')
        .next()
        .expect("No major version in required_version")
    {
        "2" => "v2.x",
        other => panic!("Please add support for bundling hwloc v{}.x", other),
    };
    let out_path = env::var("OUT_DIR").expect("No output directory given");

    // Fetch latest supported hwloc from git
    let source_path = fetch_hwloc(out_path, source_version);

    // On Windows, we build using CMake because the autotools build
    // procedure does not work with MSVC, which is often needed on this OS
    #[cfg(target_os = "windows")]
    panic!("windows not supported");

    // On other OSes, we use autotools and pkg-config
    #[cfg(not(target_os = "windows"))]
    install_hwloc_autotools(source_path);
}

/// Fetch hwloc from a git release branch, return repo path
#[cfg(feature = "bundled")]
fn fetch_hwloc(parent_path: impl AsRef<Path>, version: &str) -> PathBuf {
    // Determine location of the git repo and its parent directory
    let parent_path = parent_path.as_ref();
    let repo_path = parent_path.join("hwloc");

    // Clone the repo if this is the first time, update it with pull otherwise
    let output = if !repo_path.join("Makefile.am").exists() {
        Command::new("git")
            .args([
                "clone",
                "https://github.com/open-mpi/hwloc",
                "--depth",
                "1",
                "--branch",
                version,
            ])
            .current_dir(parent_path)
            .output()
            .expect("git clone for hwloc failed")
    } else {
        Command::new("git")
            .args(["pull", "--ff-only", "origin", "v2.x"])
            .current_dir(&repo_path)
            .output()
            .expect("git pull for hwloc failed")
    };

    // Make sure the command returned a successful status
    let status = output.status;
    assert!(
        status.success(),
        "git clone/pull for hwloc returned failure status {status}:\n{output:?}",
        status = status,
        output = output,
    );

    // Propagate repo path
    repo_path
}

/// Compile hwloc using cmake, return local installation path
#[cfg(all(feature = "bundled", windows))]
fn install_hwloc_cmake(_source_path: impl AsRef<Path>) {
    panic!("windows not supported");
}

/// Compile hwloc using autotools, return local installation path
#[cfg(all(feature = "bundled", not(windows)))]
fn install_hwloc_autotools(source_path: impl AsRef<Path>) {
    // Build using autotools
    let mut config = autotools::Config::new(source_path);

    config
        .config_option("config-cache", None)
        .disable("cuda", None)
        .disable("cairo", None)
        .disable("picky", None)
        .disable("rsmi", None)
        .disable("nvml", None)
        .disable("gl", None)
        .disable("readme", None);

    if cfg!(target_os = "macos") {
        // macOS really doesn't like static builds...
        config.disable_static();
        config.enable_shared();
    } else {
        // ...but they make life easier elsewhere
        config.enable_static();
        config.disable_shared();
    }
    let install_path = config.fast_build(true).reconf("-ivf").build();

    // Compute the associated PKG_CONFIG_PATH
    let new_path = |lib_dir: &str| install_path.join(lib_dir).join("pkgconfig");
    let new_path = format!(
        "{}:{}",
        new_path("lib").display(),
        new_path("lib64").display()
    );

    // Combine it with any pre-existing PKG_CONFIG_PATH
    match env::var("PKG_CONFIG_PATH") {
        Ok(old_path) if !old_path.is_empty() => {
            env::set_var("PKG_CONFIG_PATH", format!("{new_path}:{old_path}"))
        }
        Ok(_) | Err(env::VarError::NotPresent) => env::set_var("PKG_CONFIG_PATH", new_path),
        Err(other_err) => panic!("Failed to check PKG_CONFIG_PATH: {}", other_err),
    }

    // Configure this build to use hwloc via pkg-config
    find_hwloc(None);
}

fn find_hwloc(required_version: Option<&str>) -> pkg_config::Library {
    // Initialize pkg-config
    let mut config = pkg_config::Config::new();

    // Specify the required version range if instructed to do so
    if let Some(required_version) = required_version {
        let first_unsupported_version = match required_version
            .split('.')
            .next()
            .expect("No major version in required_version")
        {
            "2" => "3.0.0",
            other => panic!("Please add support for hwloc v{}.x", other),
        };
        config.range_version(required_version..first_unsupported_version);
    }

    // Run pkg-config
    let lib = config
        .statik(cfg!(not(target_os = "macos")))
        .probe("hwloc")
        .expect("Could not find a suitable version of hwloc");

    // As it turns-out, pkg-config does not correctly set up the RPATHs for the
    // transitive dependencies of hwloc itself in static builds. Fix that.
    if cfg!(target_family = "unix") {
        for link_path in &lib.link_paths {
            println!(
                "cargo:rustc-link-arg=-Wl,-rpath,{}",
                link_path
                    .to_str()
                    .expect("Link path is not an UTF-8 string")
            );
        }
    }

    // Forward pkg-config output for futher consumption
    lib
}
