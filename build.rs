fn main() {
    // println!("cargo:rustc-link-arg-bins=-Tlinkall.x");
    //
    // println!("cargo:rustc-link-arg-bins=-Trom_functions.x");

    println!("cargo:rerun-if-changed=.env");

    let dotenv = dotenvy::vars();
    dotenv.for_each(|kv| {
        println!("cargo:rustc-env={}={}", kv.0, kv.1);
    });

    slint_build::compile_with_config(
        "ui/main.slint",
        slint_build::CompilerConfiguration::new()
            .embed_resources(slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer)
            .with_style("cosmic".to_string()),
    )
    .unwrap();
    slint_build::print_rustc_flags().unwrap();
}
