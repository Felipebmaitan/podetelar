fn main() {
    println!("cargo:rerun-if-env-changed=FREEDOMSTREAM_UPDATE_FEED");
    println!("cargo:rerun-if-env-changed=FREEDOMSTREAM_LINK_TEST_SCHEME");
    println!("cargo:rerun-if-env-changed=FREEDOMSTREAM_LINK_TEST_ACTIVE");
    println!("cargo:rerun-if-changed=../../assets/windows/podetelar.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("../../assets/windows/podetelar.ico")
            .set("ProductName", "PodeTelar")
            .set("FileDescription", "PodeTelar installed launcher")
            .compile()
            .expect("Could not embed installed launcher resources");
    }
}
