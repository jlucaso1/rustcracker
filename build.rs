use spirv_builder::{MetadataPrintout, SpirvBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let result = SpirvBuilder::new("shader", "spirv-unknown-vulkan1.2")
        .print_metadata(MetadataPrintout::Full)
        .build()?;

    let path = result.module.unwrap_single();
    println!("cargo:rustc-env=shader.spv={}", path.display());
    println!("cargo:warning=Shader compiled to: {path:?}");

    Ok(())
}
