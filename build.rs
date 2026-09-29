fn main() {
  println!("cargo:rerun-if-changed=build.rs");
  println!("cargo:rerun-if-changed=assets/icon.ico");

  if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
    && std::env::var("CROSS_COMPILE").is_err()
  {
    if let Ok(res) = winres::WindowsResource::new()
      .set_icon("assets/icon.ico")
      .compile()
    {
      let _ = res;
    }
  }
}
