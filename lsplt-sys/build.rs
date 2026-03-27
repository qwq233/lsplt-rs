use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=wrapper.hpp");
    println!("cargo:rerun-if-changed=wrapper.cc");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=ANDROID_NDK");
    println!("cargo:rerun-if-env-changed=ANDROID_NDK_HOME");
    println!("cargo:rerun-if-env-changed=ANDROID_NDK_ROOT");
    println!("cargo:rerun-if-env-changed=NDK_HOME");
    println!("cargo:rerun-if-env-changed=CARGO_NDK_SYSROOT_PATH");
    println!("cargo:rerun-if-env-changed=CARGO_NDK_ANDROID_PLATFORM");
    println!("cargo:rerun-if-env-changed=CLANG_PATH");
    println!("cargo:rerun-if-env-changed=ANDROID_PLATFORM");

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let jni_dir = manifest_dir.join("LSPlt/lsplt/src/main/jni");
    emit_rerun_for_dir(&jni_dir);

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is not set"));
    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("CARGO_CFG_TARGET_OS is not set");

    sanitize_clang_path_env();

    if env::var("DOCS_RS").is_ok() || target_os != "android" {
        generate_bindings(&manifest_dir, &out_dir, None, None);
        return;
    }

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").expect("CARGO_CFG_TARGET_ARCH is not set");
    let target = env::var("TARGET").expect("TARGET is not set");
    let android_platform = resolve_android_platform();
    let clang_target = resolve_clang_target(&target, &android_platform);

    let ndk = resolve_ndk_root().expect(
        "Android NDK not found. Set ANDROID_NDK/ANDROID_NDK_HOME/ANDROID_NDK_ROOT/NDK_HOME, \
         or build through cargo-ndk so CARGO_NDK_SYSROOT_PATH is available.",
    );
    let prebuilt_tag = find_ndk_prebuilt_tag(&ndk).expect("Failed to locate NDK prebuilt toolchain directory");
    let sysroot = ndk
        .join("toolchains")
        .join("llvm")
        .join("prebuilt")
        .join(&prebuilt_tag)
        .join("sysroot");

    generate_bindings(&manifest_dir, &out_dir, Some(&sysroot), Some(&clang_target));

    if !jni_dir.exists() {
        run(
            Command::new("git").args(["submodule", "update", "--init", "--recursive"]),
            "init LSPlt submodule",
        );
    }

    build_lsplt(&manifest_dir, &jni_dir, &ndk, &prebuilt_tag, &clang_target, &sysroot);

    println!("cargo:rustc-link-lib=log");
    println!("cargo:rustc-link-lib=c++_static");
    println!("cargo:rustc-link-lib=c++abi");

    let clang_lib_dir = ndk
        .join("toolchains")
        .join("llvm")
        .join("prebuilt")
        .join(&prebuilt_tag)
        .join("lib")
        .join("clang")
        .join(find_clang_version(&ndk, &prebuilt_tag).expect("Failed to find bundled clang version"))
        .join("lib")
        .join("linux");

    println!(
        "cargo:rustc-link-search=native={}",
        clang_lib_dir
            .to_str()
            .expect("Non-UTF8 clang runtime library directory")
    );
    println!("cargo:rustc-link-lib=clang_rt.builtins-{target_arch}-android");
}

fn build_lsplt(
    manifest_dir: &Path,
    jni_dir: &Path,
    ndk: &Path,
    prebuilt_tag: &str,
    clang_target: &str,
    sysroot: &Path,
) {
    let mut build = cc::Build::new();
    build.cpp(true);
    build.std("c++20");
    build.compiler(
        ndk.join("toolchains")
            .join("llvm")
            .join("prebuilt")
            .join(prebuilt_tag)
            .join("bin")
            .join(clang_cxx_filename()),
    );
    build.flag(&format!("--target={clang_target}"));
    build.flag(&format!(
        "--sysroot={}",
        sysroot.to_str().expect("Non-UTF8 sysroot path")
    ));
    build.include(jni_dir);
    build.include(jni_dir.join("include"));
    build.file(manifest_dir.join("wrapper.cc"));
    build.file(jni_dir.join("lsplt.cc"));
    build.file(jni_dir.join("elf_util.cc"));
    build.cpp_link_stdlib(None);
    build.compile("lsplt_static");
}

fn generate_bindings(
    manifest_dir: &Path,
    out_dir: &Path,
    sysroot: Option<&Path>,
    clang_target: Option<&str>,
) {
    let include_dir = manifest_dir.join("LSPlt/lsplt/src/main/jni/include");

    let mut bindings = bindgen::Builder::default()
        .header(
            manifest_dir
                .join("wrapper.hpp")
                .to_str()
                .expect("Non-UTF8 wrapper.hpp path"),
        )
        .allowlist_type("lsplt.*")
        .allowlist_function("lsplt.*")
        .allowlist_var("lsplt.*")
        .opaque_type("std::.*")
        .clang_arg(format!("-I{}", include_dir.to_str().expect("Non-UTF8 include path")))
        .clang_arg("-std=c++20")
        .layout_tests(false);

    if let Some(clang_target) = clang_target {
        bindings = bindings.clang_arg(format!("--target={clang_target}"));
    }
    if let Some(sysroot) = sysroot {
        bindings = bindings.clang_arg(format!(
            "--sysroot={}",
            sysroot.to_str().expect("Non-UTF8 sysroot path")
        ));
    }

    let bindings = bindings
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("Couldn't write bindings");
}

fn sanitize_clang_path_env() {
    if let Ok(clang_path) = env::var("CLANG_PATH") {
        let path = PathBuf::from(&clang_path);
        if path.is_file() {
            return;
        }

        #[cfg(windows)]
        {
            let exe_path = path.with_extension("exe");
            if exe_path.is_file() {
                env::set_var("CLANG_PATH", &exe_path);
                println!(
                    "cargo:warning=Normalized CLANG_PATH from {clang_path:?} to {:?}",
                    exe_path
                );
                return;
            }
        }

        env::remove_var("CLANG_PATH");
        println!(
            "cargo:warning=Ignoring invalid CLANG_PATH={clang_path:?}; bindgen will fall back to automatic clang discovery"
        );
    }
}

fn resolve_android_platform() -> String {
    env::var("CARGO_NDK_ANDROID_PLATFORM")
        .or_else(|_| env::var("ANDROID_PLATFORM"))
        .map(|value| value.trim_start_matches("android-").to_string())
        .unwrap_or_else(|_| "21".to_string())
}

fn resolve_clang_target(target: &str, android_platform: &str) -> String {
    for key in [
        format!("CXXFLAGS_{target}"),
        format!("CXXFLAGS_{}", target.replace('-', "_")),
    ] {
        if let Ok(flags) = env::var(&key) {
            if let Some(value) = flags
                .split_whitespace()
                .find_map(|flag| flag.strip_prefix("--target="))
            {
                return value.to_string();
            }
        }
    }

    if target.contains("linux-android") && !target.ends_with(android_platform) {
        return format!("{target}{android_platform}");
    }

    target.to_string()
}

fn clang_cxx_filename() -> &'static str {
    #[cfg(windows)]
    {
        "clang++.exe"
    }
    #[cfg(not(windows))]
    {
        "clang++"
    }
}

fn resolve_ndk_root() -> Option<PathBuf> {
    for key in ["ANDROID_NDK", "ANDROID_NDK_HOME", "ANDROID_NDK_ROOT", "NDK_HOME"] {
        if let Some(value) = env::var_os(key) {
            let path = PathBuf::from(value);
            if path.exists() {
                return Some(path);
            }
        }
    }

    let sysroot = PathBuf::from(env::var_os("CARGO_NDK_SYSROOT_PATH")?);
    let prebuilt_dir = sysroot.parent()?;
    let llvm_dir = prebuilt_dir.parent()?.parent()?;
    let toolchains_dir = llvm_dir.parent()?;
    let ndk_root = toolchains_dir.parent()?;
    Some(ndk_root.to_path_buf())
}

fn find_ndk_prebuilt_tag(ndk_root: &Path) -> Option<String> {
    let prebuilt_root = ndk_root.join("toolchains/llvm/prebuilt");
    fs::read_dir(prebuilt_root)
        .ok()?
        .filter_map(Result::ok)
        .find_map(|entry| {
            entry.file_type().ok().filter(|ty| ty.is_dir())?;
            entry.file_name().into_string().ok()
        })
}

fn find_clang_version(ndk_root: &Path, prebuilt_tag: &str) -> Option<String> {
    let clang_root = ndk_root
        .join("toolchains/llvm/prebuilt")
        .join(prebuilt_tag)
        .join("lib/clang");
    fs::read_dir(clang_root)
        .ok()?
        .filter_map(Result::ok)
        .find_map(|entry| {
            entry.file_type().ok().filter(|ty| ty.is_dir())?;
            entry.file_name().into_string().ok()
        })
}

fn run(command: &mut Command, description: &str) {
    let status = command
        .status()
        .unwrap_or_else(|err| panic!("Failed to {description}: {err}"));
    assert!(status.success(), "Command failed while trying to {description}");
}

fn emit_rerun_for_dir(dir: &Path) {
    if !dir.exists() {
        return;
    }

    println!("cargo:rerun-if-changed={}", dir.display());
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            emit_rerun_for_dir(&path);
        } else {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}
