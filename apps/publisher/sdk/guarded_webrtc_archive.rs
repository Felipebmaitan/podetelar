// SPDX-License-Identifier: MIT
use std::{env, fs, path::Path, process::Command};

fn run(command: &mut Command) -> String {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let result = command.output().expect("run WebRTC archiver");
    assert!(
        result.status.success(),
        "WebRTC archiver failed: {} {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).expect("read WebRTC archive listing")
}

pub fn prepare(builder: &cc::Build, webrtc: &Path, target: &str) {
    let header = webrtc.join("include/api/crypto/frame_crypto_transformer.h");
    assert_eq!(
        fs::read_to_string(&header)
            .expect("read WebRTC crypto ABI")
            .replace("\r\n", "\n"),
        include_str!("frame_crypto_transformer.h").replace("\r\n", "\n"),
        "WebRTC crypto header differs from the reviewed ABI"
    );
    let windows = target == "windows";
    assert!(
        windows || target == "macos",
        "Unqualified WebRTC archive platform"
    );
    let ar = || {
        if windows {
            builder.get_archiver()
        } else {
            let mut command = Command::new("xcrun");
            command.arg("ar");
            command
        }
    };
    let (input_name, output_name, object_name) = if windows {
        (
            "webrtc.lib",
            "podetelar_webrtc.lib",
            "frame_crypto_transformer.obj",
        )
    } else {
        (
            "libwebrtc.a",
            "libpodetelar_webrtc.a",
            "frame_crypto_transformer.o",
        )
    };
    let input = webrtc.join("lib").join(input_name);
    let out = std::path::PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let output = out.join(output_name);
    let members = |file: &Path| {
        let listing = run(ar().arg(if windows { "/LIST" } else { "-t" }).arg(file));
        listing
            .lines()
            .map(str::trim)
            .filter(|name| {
                name.rsplit(['/', '\\']).next().is_some_and(|base| {
                    base == object_name || base.ends_with(&format!(".{object_name}"))
                })
            })
            .map(String::from)
            .collect::<Vec<_>>()
    };
    let original = members(&input);
    assert_eq!(
        original.len(),
        1,
        "Expected one pinned frame cryptor object"
    );
    if windows {
        run(ar()
            .arg("/NOLOGO")
            .arg(format!("/OUT:{}", output.display()))
            .arg(format!("/REMOVE:{}", original[0]))
            .arg(&input));
    } else {
        fs::copy(&input, &output).expect("copy pinned WebRTC archive");
        run(ar().arg("-d").arg(&output).arg(&original[0]));
        run(Command::new("xcrun").arg("ranlib").arg(&output));
    }
    assert!(
        members(&output).is_empty(),
        "Stock frame cryptor still present"
    );
    println!("cargo:rerun-if-changed={}", input.display());
    println!("cargo:rerun-if-changed={}", header.display());
    println!("cargo:rerun-if-changed=frame_crypto_transformer.h");
    println!("cargo:rerun-if-changed=guarded_webrtc_archive.rs");
    println!("cargo:rerun-if-changed=src/sender_nonce.h");
    println!("cargo:rerun-if-changed=src/protected_frame_payload.h");
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=podetelar_webrtc");
}
