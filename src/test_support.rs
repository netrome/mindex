use std::path::PathBuf;

pub(crate) fn create_temp_root(test_name: &str) -> PathBuf {
    let mut root = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    root.push(format!("mindex-{}-{}", test_name, nanos));
    std::fs::create_dir_all(&root).expect("create temp dir");
    root
}
