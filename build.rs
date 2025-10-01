use spirv_builder::{MetadataPrintout, SpirvBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let result = SpirvBuilder::new("shader", "spirv-unknown-vulkan1.2")
        .print_metadata(MetadataPrintout::Full)
        .build()?;

    println!(
        "cargo:warning=Shader compiled to: {:?}",
        result.module.unwrap_single()
    );

    Ok(())
}
