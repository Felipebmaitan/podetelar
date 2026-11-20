fn main() {
    println!("cargo:rerun-if-changed=../../assets/windows/podetelar.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("../../assets/windows/podetelar.ico")
            .set("ProductName", "PodeTelar")
            .set("FileDescription", "PodeTelar portable app")
            .set("LegalCopyright", "PodeTelar contributors. MIT licensed.")
            .compile()
            .expect("Could not embed Windows app resources");
    }
}
