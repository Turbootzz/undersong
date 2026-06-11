//! P10 gate: a PNG under assets/custom/ beats its generated twin.

#[test]
fn custom_art_wins_over_generated() {
    let custom = std::path::Path::new("../../assets/custom/sprites/tiles");
    std::fs::create_dir_all(custom).expect("custom dir");
    let probe = custom.join("grass.png");
    std::fs::copy("../../assets/sprites/tiles/patch.png", &probe).expect("plant probe");
    let resolved = game::art::art("sprites/tiles/grass.png");
    std::fs::remove_file(&probe).expect("cleanup");
    let _ = std::fs::remove_dir(custom);
    assert_eq!(resolved, "custom/sprites/tiles/grass.png");
    assert_eq!(
        game::art::art("sprites/tiles/grass.png"),
        "sprites/tiles/grass.png",
        "with no custom file the generated path stands"
    );
}
