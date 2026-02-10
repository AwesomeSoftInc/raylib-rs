/* raylib-sys
   build.rs - Cargo build script

Copyright (c) 2018-2019 Paul Clement (@deltaphc)

This software is provided "as-is", without any express or implied warranty. In no event will the authors be held liable for any damages arising from the use of this software.

Permission is granted to anyone to use this software for any purpose, including commercial applications, and to alter it and redistribute it freely, subject to the following restrictions:

  1. The origin of this software must not be misrepresented; you must not claim that you wrote the original software. If you use this software in a product, an acknowledgment in the product documentation would be appreciated but is not required.

  2. Altered source versions must be plainly marked as such, and must not be misrepresented as being the original software.

  3. This notice may not be removed or altered from any source distribution.
*/
#![allow(dead_code)]

extern crate bindgen;

use std::collections::HashSet;
use std::env;
use std::path::{Path, PathBuf};

/// latest version on github's release page as of time or writing
const LATEST_RAYLIB_VERSION: &str = "5.0.0";
const LATEST_RAYLIB_API_VERSION: &str = "5";

#[derive(Debug)]
struct IgnoreMacros(HashSet<String>);

impl bindgen::callbacks::ParseCallbacks for IgnoreMacros {
    fn will_parse_macro(&self, name: &str) -> bindgen::callbacks::MacroParsingBehavior {
        if self.0.contains(name) {
            bindgen::callbacks::MacroParsingBehavior::Ignore
        } else {
            bindgen::callbacks::MacroParsingBehavior::Default
        }
    }
}

fn gen_bindings() {
    let target = env::var("TARGET").expect("Cargo build scripts always have TARGET");
    let (platform, os) = platform_from_target(&target);

    let plat = match platform {
        Platform::Desktop => "-DPLATFORM_DESKTOP",
        Platform::RPI => "-DPLATFORM_RPI",
        Platform::Android => "-DPLATFORM_ANDROID",
        Platform::Web => "-DPLATFORM_WEB",
    };

    let ignored_macros = IgnoreMacros(
        vec![
            "FP_INFINITE".into(),
            "FP_NAN".into(),
            "FP_NORMAL".into(),
            "FP_SUBNORMAL".into(),
            "FP_ZERO".into(),
            "IPPORT_RESERVED".into(),
        ]
        .into_iter()
        .collect(),
    );

    let mut builder = bindgen::Builder::default()
        .header("binding/binding.h")
        .rustified_enum(".+")
        .clang_arg("-std=c99")
        .clang_arg(plat)
        .parse_callbacks(Box::new(ignored_macros));

    #[cfg(feature = "imgui")]
    {
        builder = builder
            .clang_arg("-I./binding/imgui/decoy")
            .clang_arg("-I./raylib/src")
            .header("binding/rlImGui/rlImGui.h");
    }

    if platform == Platform::Desktop && os == PlatformOS::Windows {
        // odd workaround for booleans being broken
        builder = builder.clang_arg("-D__STDC__");
    }

    if platform == Platform::Web {
        builder = builder
            .clang_arg("-fvisibility=default")
            .clang_arg("--target=wasm32-emscripten");
    }

    // Build
    let bindings = builder.generate().expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}

fn gen_rgui() {
    // Compile the code and link with cc crate
    #[cfg(target_os = "windows")]
    {
        cc::Build::new()
            .files(vec!["binding/rgui_wrapper.cpp", "binding/utils_log.cpp"])
            .include("binding")
            .warnings(false)
            // .flag("-std=c99")
            .extra_warnings(false)
            .compile("rgui");
    }
    #[cfg(not(target_os = "windows"))]
    {
        cc::Build::new()
            .files(vec!["binding/rgui_wrapper.c", "binding/utils_log.c"])
            .include("binding")
            .warnings(false)
            // .flag("-std=c99")
            .extra_warnings(false)
            .compile("rgui");
    }
}

fn gen_imgui() {
    println!("cargo:rustc-link-lib=dylib=stdc++");

    cc::Build::new()
        .define("NO_FONT_AWESOME", "1")
        .files(vec!["binding/rlImGui/rlImGui.cpp"])
        .include("binding/imgui")
        .include("raylib/src")
        .warnings(false)
        .extra_warnings(false)
        .compile("rlImGui");
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=./binding/binding.h");
    let target = env::var("TARGET").expect("Cargo build scripts always have TARGET");

    // make sure cmake knows that it should bundle glfw in
    if target.contains("wasm") {
        if let Err(e) = env::var("EMCC_CFLAGS") {
            if e == std::env::VarError::NotPresent {
                panic!("\nYou must set the following environment variables yourself to compile for WASM. We are sorry for the inconvienence; this will be fixed in 5.1.0.\n{}{}\"\n",{
                    #[cfg(target_family = "windows")]
                    {"Paste this before executing the command: set EMCC_CFLAGS="}
                    #[cfg(not(target_family = "windows"))]
                    {"Prefix your command with this (you may want to make a build script for obvious reasons...): EMCC_CFLAGS="}
                },"\"-O3 -sUSE_GLFW=3 -sGL_ENABLE_GET_PROC_ADDRESS -sWASM=1 -sALLOW_MEMORY_GROWTH=1 -sWASM_MEM_MAX=512MB -sTOTAL_MEMORY=512MB -sABORTING_MALLOC=0 -sASYNCIFY -sFORCE_FILESYSTEM=1 -sASSERTIONS=1 -sERROR_ON_UNDEFINED_SYMBOLS=0 -sEXPORTED_RUNTIME_METHODS=ccallcwrap\"");
            } else {
                panic!("\nError regarding EMCC_CFLAGS: {:?}\n", e);
            }
        }
    }

    let raylib_src = "./raylib";
    if is_directory_empty(raylib_src) {
        panic!("raylib source does not exist in: `raylib-sys/raylib`. Please copy it in");
    }

    gen_bindings();

    gen_rgui();

    gen_imgui();
}

#[must_use]
/// returns false if the directory does not exist
fn is_directory_empty(path: &str) -> bool {
    match std::fs::read_dir(path) {
        Ok(mut dir) => dir.next().is_none(),
        Err(_) => true,
    }
}

// run_command runs a command to completion or panics. Used for running curl and powershell.
fn run_command(cmd: &str, args: &[&str]) {
    use std::process::Command;
    match Command::new(cmd).args(args).output() {
        Ok(output) => {
            if !output.status.success() {
                let error = std::str::from_utf8(&output.stderr).unwrap();
                panic!("Command '{}' failed: {}", cmd, error);
            }
        }
        Err(error) => {
            panic!("Error running command '{}': {:#}", cmd, error);
        }
    }
}

fn platform_from_target(target: &str) -> (Platform, PlatformOS) {
    let platform = if target.contains("wasm") {
        Platform::Web
    } else if target.contains("armv7-unknown-linux") {
        Platform::RPI
    } else if target.contains("android") {
        Platform::Android
    } else {
        Platform::Desktop
    };

    let platform_os = if platform == Platform::Desktop {
        // Determine PLATFORM_OS in case PLATFORM_DESKTOP selected
        if env::var("OS")
            .unwrap_or("".to_owned())
            .contains("Windows_NT")
            || env::var("TARGET")
                .unwrap_or("".to_owned())
                .contains("windows")
        {
            // No uname.exe on MinGW!, but OS=Windows_NT on Windows!
            // ifeq ($(UNAME),Msys) -> Windows
            PlatformOS::Windows
        } else {
            let un: &str = &uname();
            match un {
                "Linux" => PlatformOS::Linux,
                "FreeBSD" => PlatformOS::BSD,
                "OpenBSD" => PlatformOS::BSD,
                "NetBSD" => PlatformOS::BSD,
                "DragonFly" => PlatformOS::BSD,
                "Darwin" => PlatformOS::OSX,
                _ => panic!("Unknown platform {}", uname()),
            }
        }
    } else if platform == Platform::RPI {
        let un: &str = &uname();
        if un == "Linux" {
            PlatformOS::Linux
        } else {
            PlatformOS::Unknown
        }
    } else {
        PlatformOS::Unknown
    };

    (platform, platform_os)
}

fn uname() -> String {
    use std::process::Command;
    String::from_utf8_lossy(
        &Command::new("uname")
            .output()
            .expect("failed to run uname")
            .stdout,
    )
    .trim()
    .to_owned()
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Platform {
    Web,
    Desktop,
    Android,
    RPI, // raspberry pi
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PlatformOS {
    Windows,
    Linux,
    BSD,
    OSX,
    Unknown,
}

#[derive(Debug, PartialEq)]
enum LibType {
    Static,
    _Shared,
}

#[derive(Debug, PartialEq)]
enum BuildMode {
    Release,
    Debug,
}

struct BuildSettings {
    pub platform: Platform,
    pub platform_os: PlatformOS,
    pub bundled_glfw: bool,
}
