//! Art-path resolution (doc 05 v2 §2): a PNG at `assets/custom/<rel>`
//! always beats the generated one. Pure std — the presenter calls it,
//! tests prove it.

/// Resolves `rel` against the custom-override tree. The web build skips
/// the probe (no filesystem) and uses generated art.
pub fn art(rel: &str) -> String {
    #[cfg(not(target_arch = "wasm32"))]
    {
        for root in ["assets", "../../assets"] {
            if std::path::Path::new(root).join("custom").join(rel).exists() {
                return format!("custom/{rel}");
            }
        }
    }
    rel.to_string()
}
