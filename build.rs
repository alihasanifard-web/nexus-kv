fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("windows") {
        let res_file = std::env::var("NEXUS_ICON_RES").unwrap_or_else(|_| "assets/server.res".to_string());
        println!("cargo:rustc-link-arg={}", res_file);
        println!("cargo:rerun-if-env-changed=NEXUS_ICON_RES");
    }
}
