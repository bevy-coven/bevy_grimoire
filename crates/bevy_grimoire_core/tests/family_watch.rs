//! **A family settings file changing on disk is noticed within a second** (#96's hot reload).

use bevy_grimoire_core::family::{FamilyWatch, SettingsChange};

fn settled(w: &mut FamilyWatch, from: f64) -> (Vec<SettingsChange>, f64) {
    // 0.5 s polls: one sees the change, the next one (0.5 s later) finds it settled.
    let mut t = from;
    let mut all = Vec::new();
    for _ in 0..3 {
        t += 0.5;
        all.extend(w.poll(t));
    }
    (all, t)
}

#[test]
fn an_edit_is_reported_once_by_crate_within_a_second_and_a_bad_edit_keeps_the_good_settings() {
    let root = std::env::temp_dir().join(format!("family_watch_{}", std::process::id()));
    let dir = root.join("family");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("bevy_elements.yaml"), "sun: {hour: 6.0}").unwrap();
    let mut w = FamilyWatch::new(&root);
    assert!(settled(&mut w, 0.0).0.is_empty(), "an untouched file is no change");

    std::thread::sleep(std::time::Duration::from_millis(30));
    std::fs::write(dir.join("bevy_elements.yaml"), "sun: {hour: 18.5}").unwrap();
    // Change seen at the first poll, due at the second: a second at 0.5 s polls.
    assert!(w.poll(10.5).is_empty());
    let got = w.poll(11.0);
    assert_eq!(got, vec![SettingsChange::Reloaded { krate: "bevy_elements".into(), text: "sun: {hour: 18.5}".into() }]);
    assert!(settled(&mut w, 11.0).0.is_empty(), "reported once");

    std::thread::sleep(std::time::Duration::from_millis(30));
    std::fs::write(dir.join("bevy_elements.yaml"), "sun: {hour: ").unwrap();
    let (got, t) = settled(&mut w, 20.0);
    assert!(matches!(got.as_slice(), [SettingsChange::Invalid { krate, .. }] if krate == "bevy_elements"), "{got:?}");

    // Restoring the last good text is not a change: the good settings were never replaced.
    std::thread::sleep(std::time::Duration::from_millis(30));
    std::fs::write(dir.join("bevy_elements.yaml"), "sun: {hour: 18.5}").unwrap();
    assert!(settled(&mut w, t).0.is_empty());

    std::fs::remove_file(dir.join("bevy_elements.yaml")).unwrap();
    let (got, _) = settled(&mut w, t + 5.0);
    assert_eq!(got, vec![SettingsChange::Removed { krate: "bevy_elements".into() }]);
    let _ = std::fs::remove_dir_all(&root);
}
