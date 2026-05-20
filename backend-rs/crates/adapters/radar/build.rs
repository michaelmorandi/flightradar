fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use the vendored protoc so the build does not require a system install.
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    std::env::set_var("PROTOC", protoc);

    let descriptor_set_out =
        std::path::PathBuf::from(std::env::var("OUT_DIR")?).join("adsb_descriptor.bin");

    tonic_build::configure()
        .build_server(true) // enables in-process server for tests
        .build_client(true)
        .file_descriptor_set_path(descriptor_set_out)
        .compile_protos(&["proto/adsb.proto"], &["proto"])?;

    println!("cargo:rerun-if-changed=proto/adsb.proto");
    Ok(())
}
