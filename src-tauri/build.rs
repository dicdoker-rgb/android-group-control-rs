fn main() {
    tauri_build::build();
    println!("cargo:rustc-env=APP_TARGET_TRIPLE={}", std::env::var("TARGET").unwrap_or_default());
}
