use embed_manifest::{embed_manifest, new_manifest};
use embed_manifest::manifest::DpiAwareness;

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        let manifest = new_manifest("Prism.Browser")
            .dpi_awareness(DpiAwareness::Unaware);

        embed_manifest(manifest).expect("unable to embed manifest file");
    }

    println!("cargo:rerun-if-changed=build.rs");
}
