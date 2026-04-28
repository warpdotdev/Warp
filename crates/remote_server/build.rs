fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/remote_server.proto");
    prost_build::compile_protos(&["proto/remote_server.proto"], &["proto/"])?;
    Ok(())
}
