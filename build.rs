fn main() {
    // println!("cargo:rustc-link-arg-bins=-Tlinkall.x");
    //
    // println!("cargo:rustc-link-arg-bins=-Trom_functions.x");

    println!("cargo:rerun-if-changed=.env");

    let dotenv = dotenvy::vars();
    dotenv.for_each(|kv| {
        println!("cargo:rustc-env={}={}", kv.0, kv.1);
    });
}
