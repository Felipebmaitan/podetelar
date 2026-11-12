fn main() {
    println!("cargo:rerun-if-env-changed=FREEDOMSTREAM_UPDATE_FEED");
    println!("cargo:rerun-if-changed=../../assets/windows/freedomstream.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("../../assets/windows/freedomstream.ico")
            .set("ProductName", "FreedomStream")
            .set("FileDescription", "FreedomStream installed launcher")
            .compile()
            .expect("Could not embed installed launcher resources");
    }
}
