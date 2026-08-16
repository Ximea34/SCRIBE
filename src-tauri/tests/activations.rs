use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use scribe_lib::domain::activations::{self, ActivationRecord};

fn temp_dir() -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let path = std::env::temp_dir().join(format!(
        "scribe-activations-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).expect("temp dir");
    path
}

fn record(callsign: &str, eobt: &str) -> ActivationRecord {
    ActivationRecord {
        callsign: callsign.to_owned(),
        dep: "LFLL".to_owned(),
        eobt: eobt.to_owned(),
        activated_at_unix_ms: 1_760_000_000_000,
    }
}

fn leftovers(directory: &PathBuf) -> Vec<String> {
    std::fs::read_dir(directory)
        .expect("readable")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name != "activations.json")
        .collect()
}

#[test]
fn activations_round_trip_through_the_file() {
    let directory = temp_dir();
    let path = directory.join("activations.json");
    let saved = vec![record("AFR1234", "1215"), record("RYR33EK", "0800")];

    activations::save(&path, &saved).expect("save");
    assert_eq!(activations::load(&path), saved);

    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn a_missing_file_restores_nothing_rather_than_failing() {
    let directory = temp_dir();
    assert!(activations::load(&directory.join("activations.json")).is_empty());
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn an_unreadable_file_restores_nothing_rather_than_failing() {
    let directory = temp_dir();
    let path = directory.join("activations.json");
    std::fs::write(&path, "{ this is not the file you are looking for").expect("write");

    assert!(activations::load(&path).is_empty());
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn the_directory_is_created_on_first_save() {
    let directory = temp_dir().join("nested").join("deeper");
    let path = directory.join("activations.json");

    activations::save(&path, &[record("AFR1234", "1215")]).expect("save");
    assert_eq!(activations::load(&path).len(), 1);

    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn overlapping_saves_do_not_collide_and_leave_nothing_behind() {
    let directory = temp_dir();
    let path = directory.join("activations.json");

    // Each save takes its own temporary name, so concurrent writers cannot delete each other's.
    let handles: Vec<_> = (0..8)
        .map(|index| {
            let path = path.clone();
            std::thread::spawn(move || {
                activations::save(&path, &[record("AFR1234", &format!("12{index:02}"))])
            })
        })
        .collect();

    for handle in handles {
        handle
            .join()
            .expect("thread")
            .expect("every concurrent save should succeed");
    }

    assert_eq!(activations::load(&path).len(), 1);
    assert!(
        leftovers(&directory).is_empty(),
        "no temporary files should survive: {:?}",
        leftovers(&directory)
    );

    let _ = std::fs::remove_dir_all(&directory);
}
