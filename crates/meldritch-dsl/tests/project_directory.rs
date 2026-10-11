use meldritch_dsl::{load_project_directory, load_song_directory};
use std::fs;
use std::path::{Path, PathBuf};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "meldritch-project-opening-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn example(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../songs/examples")
        .join(name)
}

#[test]
fn empty_directory_is_a_valid_read_only_project_without_manifest_or_git() {
    let directory = Directory::new();
    let project = load_project_directory(&directory.0).unwrap();
    assert_eq!(project.root(), directory.0.canonicalize().unwrap());
    assert!(project.composition().is_none());
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
    assert!(load_song_directory(&directory.0).is_err());
    fs::write(directory.0.join("README.md"), "My musical project\n").unwrap();
    assert!(
        load_project_directory(&directory.0)
            .unwrap()
            .composition()
            .is_none()
    );
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
fn legacy_entry_uses_the_same_semantic_loader_and_fingerprint() {
    for name in [
        "09-curated-performance-controls",
        "29-lua-authored-roundtrip",
        "33-plaits-models",
    ] {
        let root = example(name);
        let base = load_song_directory(&root).unwrap();
        let project = load_project_directory(&root).unwrap();
        assert_eq!(project.composition(), Some(&base));
        assert_eq!(project.root(), base.root());
        assert_eq!(project.authoring_source(), base);
    }
}

#[test]
fn empty_palette_construction_reuses_typed_batches_without_creating_timed_material() {
    use meldritch_dsl::{PatchCreation, SignalType};
    let directory = Directory::new();
    let empty = load_project_directory(&directory.0)
        .unwrap()
        .authoring_source();
    assert!(empty.synths().is_empty());
    assert!(empty.performance().tracks().is_empty());
    assert!(empty.note_patterns().is_empty());
    let request: PatchCreation = toml::from_str(
        &fs::read_to_string(example("34-empty-project-patch/construction.toml")).unwrap(),
    )
    .unwrap();
    assert_eq!(request, PatchCreation::mono_voice("voice-1"));
    let source = empty.create_patch(&request).unwrap();
    assert_ne!(source.fingerprint(), empty.fingerprint());
    assert_eq!(source, empty.create_patch(&request).unwrap());
    let graph = &source.synths()["voice-1"];
    assert_eq!(graph.modules().len(), 4);
    assert_eq!(graph.cables().len(), 5);
    assert_eq!(graph.inputs()[0].signal(), SignalType::Pitch);
    assert_eq!(graph.inputs()[1].signal(), SignalType::Gate);
    assert!(source.performance().tracks().is_empty());
    assert!(source.note_patterns().is_empty());
    assert!(
        source
            .create_patch(&request)
            .unwrap_err()
            .contains("duplicate instrument")
    );
    for name in [
        "incomplete-construction.toml",
        "incompatible-construction.toml",
    ] {
        let invalid: PatchCreation = toml::from_str(
            &fs::read_to_string(example(&format!("34-empty-project-patch/invalid/{name}")))
                .unwrap(),
        )
        .unwrap();
        assert!(empty.create_patch(&invalid).is_err(), "{name}");
    }
    assert!(empty.synths().is_empty());
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
    let mut duplicate_inputs = request.clone();
    duplicate_inputs.inputs.push(request.inputs[0].clone());
    assert!(empty.create_patch(&duplicate_inputs).is_err());
    assert!(
        empty
            .create_patch(&PatchCreation::mono_voice("../escape"))
            .is_err()
    );
}

#[test]
fn malformed_existing_entry_and_invalid_roots_never_become_empty_projects() {
    let directory = Directory::new();
    fs::copy(
        example("34-empty-project-patch/invalid/malformed-entry.toml"),
        directory.0.join("main.mlperformance"),
    )
    .unwrap();
    let error = load_project_directory(&directory.0)
        .unwrap_err()
        .to_string();
    assert!(error.contains("main.mlperformance"), "{error}");
    assert!(load_project_directory(directory.0.join("missing")).is_err());
    assert!(load_project_directory(directory.0.join("main.mlperformance")).is_err());
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn dangling_and_escaping_entry_symlinks_are_diagnosed() {
    use std::os::unix::fs::symlink;
    let directory = Directory::new();
    let outside = Directory::new();
    let entry = directory.0.join("main.mlperformance");
    symlink(outside.0.join("missing"), &entry).unwrap();
    assert!(load_project_directory(&directory.0).is_err());
    fs::remove_file(&entry).unwrap();
    fs::write(outside.0.join("entry"), "not a project").unwrap();
    symlink(outside.0.join("entry"), &entry).unwrap();
    let error = load_project_directory(&directory.0)
        .unwrap_err()
        .to_string();
    assert!(error.contains("escapes project root"), "{error}");
}
