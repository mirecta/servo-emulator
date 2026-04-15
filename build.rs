fn main() {
    embuild::espidf::sysenv::output();
    println!("cargo::rustc-check-cfg=cfg(esp32)");
    println!("cargo::rustc-check-cfg=cfg(esp32s3)");
}
