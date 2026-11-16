fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        // WebRTC NSString categories live in otherwise unreferenced ObjC objects.
        println!("cargo:rustc-link-arg=-Wl,-ObjC");
        println!("cargo:rustc-link-arg=-Wl,-headerpad_max_install_names");
    }
    println!("cargo:rerun-if-changed=../../assets/windows/podetelar.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("../../assets/windows/podetelar.ico")
            .set("ProductName", "PodeTelar")
            .set("FileDescription", "PodeTelar")
            .set("LegalCopyright", "PodeTelar contributors. MIT licensed.")
            .compile()
            .expect("Could not embed Windows app resources");
    }
}
