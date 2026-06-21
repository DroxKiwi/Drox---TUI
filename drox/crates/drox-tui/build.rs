//! Ressources Windows (icône exe) — voir `packaging/assets/drox.ico`.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let icon = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../packaging/assets/drox.ico");
    if !icon.is_file() {
        println!(
            "cargo:warning=icone Windows absente ({}), binaire sans icone embarquee",
            icon.display()
        );
        return;
    }

    let mut res = winres::WindowsResource::new();
    res.set_icon(icon.to_str().expect("chemin icone UTF-8"));
    res.compile().expect("compilation ressources Windows (winres)");
}
