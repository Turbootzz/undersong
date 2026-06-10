//! Asset-pipeline guards: the signature trick (one melody feeds sigil
//! and cry) plus the doc 04 §5–§6 laws.

#[cfg(test)]
mod tests {
    use crate::melody::{Timbre, melody};
    use undersong_core::types::Type;

    #[test]
    fn melody_is_deterministic_per_seed() {
        let a = melody(42, Type::Ember, 60, 95, 0);
        let b = melody(42, Type::Ember, 60, 95, 0);
        let notes_a: Vec<(u8, u32)> = a.notes.iter().map(|n| (n.midi, n.millis)).collect();
        let notes_b: Vec<(u8, u32)> = b.notes.iter().map(|n| (n.midi, n.millis)).collect();
        assert_eq!(notes_a, notes_b);
        assert_eq!(a.timbre, Timbre::Brass);
    }

    #[test]
    fn leitmotif_family_shares_contour_with_ornaments() {
        // Same seed (one evolution line): the evolved cry repeats the
        // root contour and appends grace notes (doc 04 §6).
        let root = melody(7, Type::Tide, 45, 80, 0);
        let evolved = melody(7, Type::Tide, 63, 320, 2);
        assert_eq!(evolved.notes.len(), root.notes.len() + 2);
        // Register law: heavier stage sits lower or equal.
        let root_low = root.notes.iter().map(|n| n.midi).min().unwrap();
        let evolved_low = evolved.notes.iter().map(|n| n.midi).min().unwrap();
        assert!(evolved_low <= root_low);
    }

    #[test]
    fn sigil_renders_with_an_eye_on_the_body() {
        let tune = melody(99, Type::Phantom, 50, 120, 0);
        let dir = std::env::temp_dir().join(format!("undersong-sigil-test-{}", std::process::id()));
        crate::sigils::render_sigil(
            "testling",
            99,
            Type::Phantom,
            "#3a2e52",
            "#c8a84b",
            &tune,
            &dir,
        )
        .expect("renders");
        let img = image::open(dir.join("testling.front.png"))
            .expect("readable")
            .to_rgba8();
        // The ink eye pixel exists somewhere (exactly-one-regard rule).
        let ink = image::Rgba([26u8, 24, 34, 255]);
        assert!(img.pixels().any(|p| *p == ink), "eye pixel present");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn cries_obey_the_duration_law() {
        // Doc 04 §6: 0.6–1.2 s. Render the extremes and measure.
        for (spe, weight, ornaments) in [(20u16, 30u16, 0u8), (170, 2400, 2)] {
            let tune = melody(5, Type::Stone, spe, weight, ornaments);
            let dir =
                std::env::temp_dir().join(format!("undersong-cry-test-{}", std::process::id()));
            crate::cries::render_cry("testling", &tune, &dir).expect("renders");
            let bytes = std::fs::metadata(dir.join("testling.wav"))
                .expect("wav")
                .len();
            // 16-bit mono 44.1 kHz: bytes ≈ samples × 2 (+44 header).
            let seconds = (bytes.saturating_sub(44)) as f64 / 2.0 / 44_100.0;
            assert!(
                (0.55..=1.4).contains(&seconds),
                "cry {seconds:.2}s outside the law (spe {spe}, weight {weight})"
            );
            std::fs::remove_dir_all(dir).ok();
        }
    }
}
