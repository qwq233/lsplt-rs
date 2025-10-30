use std::env;
use std::io::BufRead;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=wrapper.hpp");
    println!("cargo:rerun-if-changed=wrapper.cc");
    println!("cargo:rerun-if-changed=build.rs");

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let abi = match target_arch.as_str() {
        "x86" => "x86",
        "x86_64" => "x86_64",
        "aarch64" => "arm64-v8a",
        "arm" => "armeabi-v7a",
        other => panic!("Unsupported target arch: {}", other),
    };

    println!("Building for target arch: {}", abi);

    let dep_dir = env!("CARGO_MANIFEST_DIR");
    let ndk = env::var("ANDROID_NDK");
    let out = env::var("OUT_DIR").unwrap();

    
    let bindings = bindgen::Builder::default()
        .header("wrapper.hpp")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Allowlist only the lsplt symbols
        .allowlist_type("lsplt.*")
        .allowlist_function("lsplt.*")
        .allowlist_var("lsplt.*")
        .opaque_type("std::.*")
        .clang_arg("-D__ANDROID_API__=21")
        .clang_arg("-std=c++20")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    if env::var("DOCS_RS").is_ok() {
        return;
    }

    let ndk = ndk.expect("ANDROID_NDK environment variable not set");

    let src = std::fs::read_dir(format!("{}/LSPlt/lsplt/src/main/jni/", dep_dir));
    if src.is_err() {
        Command::new("git")
            .args(["submodule", "update", "--init", "--recursive"])
            .status()
            .expect("Failed to init submodule");
    }

    // clean old build dir
    let _ = std::fs::remove_dir_all(format!("{}/build/{}", out, abi));

    std::fs::create_dir_all(format!("{}/build/src", out))
        .expect("Failed to create build directory");

    let src = format!("{}/build/src", out);
    copy_dir_all(format!("{}/LSPlt/lsplt/src/main/jni/", dep_dir), &src)
        .expect("Failed to copy source files");

    std::fs::copy(
        format!("{}/wrapper.cc", dep_dir),
        format!("{}/wrapper.cc", src),
    )
    .expect("Failed to copy wrapper.cc");
    std::fs::copy(
        format!("{}/wrapper.hpp", dep_dir),
        format!("{}/include/wrapper.hpp", src),
    )
    .expect("Failed to copy wrapper.hpp");
    let mut cmakelist = std::fs::File::open(format!("{}/CMakeLists.txt", src))
        .expect("Failed to open CMakeLists.txt");

    let content = std::io::BufReader::new(&mut cmakelist)
        .lines()
        .collect::<Result<Vec<_>, _>>()
        .expect("Failed to read CMakeLists.txt")
        .join("\n");
    let content = content.replace("set(SOURCES", "set(SOURCES wrapper.cc");
    std::fs::write(format!("{}/CMakeLists.txt", src), content)
        .expect("Failed to write CMakeLists.txt");

    Command::new("cmake")
        .args([
            "-S",
            src.as_str(),
            "-B",
            &format!("{}/build/{}", out, abi),
            "-DCMAKE_BUILD_TYPE=Release",
            "-DCMAKE_POLICY_VERSION_MINIMUM=3.5",
            &format!(
                "-DCMAKE_SYSROOT={}/toolchains/llvm/prebuilt/linux-x86_64/sysroot/",
                ndk
            ),
            &format!(
                "-DCMAKE_TOOLCHAIN_FILE={}/build/cmake/android.toolchain.cmake",
                ndk
            ),
            &format!("-DANDROID_ABI={}", abi),
            "-DANDROID_PLATFORM=android-21",
            "-DANDROID_STL=c++_shared",
        ])
        .status()
        .expect("Failed to run cmake");

    Command::new("cmake")
        .args([
            "--build",
            &format!("{}/build/{}", out, abi),
            "--target",
            "lsplt_static",
        ])
        .status()
        .expect("Failed to build lsplt_static");

    println!(
        "cargo:rustc-link-search=native={}/build/{}",
        out, abi
    );
    println!("cargo:rustc-link-lib=lsplt_static");
    println!("cargo:rustc-link-lib=c++_shared");

    // fix __builtin___clear_cache symbol not found
    let clang_lib_dir = format!(
        "toolchains/llvm/prebuilt/linux-x86_64/lib/clang/{}/lib/linux/",
        std::fs::read_dir(format!("{}/toolchains/llvm/prebuilt/linux-x86_64/lib/clang/", ndk))
            .expect("Failed to read clang version")
            .next()
            .expect("No clang version found")
            .unwrap()
            .file_name()
            .into_string()
            .unwrap()
    );
    println!("cargo:rustc-link-search={ndk}/{clang_lib_dir}");
    println!("cargo:rustc-link-lib=clang_rt.builtins-{target_arch}-android");
}

fn copy_dir_all(
    src: impl AsRef<std::path::Path>,
    dst: impl AsRef<std::path::Path>,
) -> std::io::Result<()> {
    std::fs::create_dir_all(&dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}
