use std::{env, fs, path::PathBuf};

fn main() {
    let config = slint_build::CompilerConfiguration::new()
        // Fonts imported in .slint are embedded, so the device needs no system fonts.
        .embed_resources(slint_build::EmbedResourcesKind::EmbedFiles)
        // Interface translations, one folder per language (see i18n/).
        .with_bundled_translations("../../i18n")
        // One flat catalogue: no per-component context in i18n/*.po.
        .with_default_translation_context(slint_build::DefaultTranslationContext::None)
        // Debug info lets tests find elements by accessible label; keep it out
        // of release builds, the printer binary has a size budget.
        .with_debug_info(env::var("PROFILE").is_ok_and(|p| p != "release"));
    slint_build::compile_with_config("../../ui/app.slint", config).expect("compile .slint UI");

    musl_libdl_stub();
}

/// Font loading may reference `-ldl` (dlopen). musl has dlopen in libc and
/// ships `libdl.a` only as an empty placeholder, which Rust's self-contained
/// musl target lacks. Provide that empty archive for static musl builds.
fn musl_libdl_stub() {
    if !env::var("TARGET").unwrap_or_default().contains("musl") {
        return;
    }
    let dir = PathBuf::from(env::var("OUT_DIR").unwrap()).join("musl-stubs");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("libdl.a"), b"!<arch>\n").unwrap();
    println!("cargo:rustc-link-search=native={}", dir.display());
}
