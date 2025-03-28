fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Tell cargo to rerun this if proto files change
    let proto_dir = ["src/proto"];
    let target_proto = "src/proto/service.proto";
    println!("cargo:rerun-if-changed={}", target_proto);

    // Configure tonic build
    tonic_build::configure()
        .out_dir("src/generated")
        .compile(&[target_proto], &proto_dir)
        .map_err(|e| format!("Failed to compile proto files: {}", e))?;

    Ok(())
}
