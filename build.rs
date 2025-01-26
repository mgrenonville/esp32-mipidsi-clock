fn main() {
    // println!("cargo:rustc-link-arg-bins=-Tlinkall.x");
    //
    // println!("cargo:rustc-link-arg-bins=-Trom_functions.x");

    slint_build::compile_with_config(
        "ui/main.slint",
        slint_build::CompilerConfiguration::new()
            .embed_resources(slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer)
            .with_style("cosmic".to_string()),
    )
    .unwrap();
    slint_build::print_rustc_flags().unwrap();
}
