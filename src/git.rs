use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

pub(crate) fn git_dir_within_root(root: &Path) -> std::io::Result<Option<PathBuf>> {
    let root = std::fs::canonicalize(root)?;
    let dot_git = root.join(".git");
    let metadata = match std::fs::symlink_metadata(&dot_git) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };

    let resolved_dot_git = std::fs::canonicalize(&dot_git)?;
    if !resolved_dot_git.starts_with(&root) {
        return Ok(None);
    }

    let resolved_meta = if metadata.file_type().is_symlink() {
        std::fs::metadata(&resolved_dot_git)?
    } else {
        metadata
    };

    if resolved_meta.is_dir() {
        return Ok(Some(resolved_dot_git));
    }

    if !resolved_meta.is_file() {
        return Ok(None);
    }

    let gitdir = match parse_gitdir_path(&resolved_dot_git)? {
        Some(path) => path,
        None => return Ok(None),
    };
    let gitdir = if gitdir.is_absolute() {
        gitdir
    } else {
        let base = match resolved_dot_git.parent() {
            Some(base) => base,
            None => return Ok(None),
        };
        base.join(gitdir)
    };

    let resolved_gitdir = match std::fs::canonicalize(&gitdir) {
        Ok(path) => path,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    if !resolved_gitdir.starts_with(&root) {
        return Ok(None);
    }
    let gitdir_meta = match std::fs::metadata(&resolved_gitdir) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    if !gitdir_meta.is_dir() {
        return Ok(None);
    }

    Ok(Some(resolved_gitdir))
}

pub(crate) struct GitSnapshot {
    pub(crate) changed_files: usize,
    pub(crate) diff: String,
}

pub(crate) struct GitCommit {
    pub(crate) id: String,
}

pub(crate) struct GitAuthor {
    pub(crate) name: String,
    pub(crate) email: String,
}

#[derive(Debug)]
pub(crate) struct GitError(String);

impl GitError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for GitError {}

pub(crate) fn git_status_and_diff(root: &Path) -> Result<GitSnapshot, GitError> {
    if !git_has_head(root)? {
        return Ok(GitSnapshot {
            changed_files: 0,
            diff: String::new(),
        });
    }

    let changed_files = git_diff_name_only(root)?;
    let diff = git_diff(root)?;

    Ok(GitSnapshot {
        changed_files,
        diff,
    })
}

pub(crate) fn git_commit_all(
    root: &Path,
    message: &str,
    author: Option<GitAuthor>,
) -> Result<GitCommit, GitError> {
    let root = std::fs::canonicalize(root)
        .map_err(|err| GitError::new(format!("canonicalize root: {err}")))?;
    let root = root.as_path();

    git_add_all(root)?;
    ensure_no_conflicts(root)?;

    if git_staged_files(root)? == 0 {
        return Err(GitError::new("no changes to commit"));
    }

    git_commit(root, message, author)?;
    let id = git_rev_parse_head(root)?;

    Ok(GitCommit { id })
}

fn git_add_all(root: &Path) -> Result<(), GitError> {
    let mut cmd = git_command(root)?;
    cmd.args(["add", "-A", "--", "."]);
    run_command_checked("git add", cmd, None)?;
    Ok(())
}

fn ensure_no_conflicts(root: &Path) -> Result<(), GitError> {
    let mut cmd = git_command(root)?;
    cmd.args([
        "diff",
        "--name-only",
        "--diff-filter=U",
        "--ignore-submodules=all",
        "--no-ext-diff",
        "--",
    ]);
    let output = run_command_checked("git diff --diff-filter=U", cmd, None)?;
    if has_non_empty_lines(&output.stdout) {
        return Err(GitError::new("conflicted index entries are not supported"));
    }
    Ok(())
}

fn git_staged_files(root: &Path) -> Result<usize, GitError> {
    let mut cmd = git_command(root)?;
    cmd.args([
        "diff",
        "--cached",
        "--name-only",
        "--ignore-submodules=all",
        "--no-ext-diff",
        "--no-color",
        "--",
    ]);
    let output = run_command_checked("git diff --cached --name-only", cmd, None)?;
    Ok(count_non_empty_lines(&output.stdout))
}

fn git_commit(root: &Path, message: &str, author: Option<GitAuthor>) -> Result<(), GitError> {
    let mut cmd = git_command(root)?;
    cmd.args(["commit", "--no-verify", "--no-gpg-sign", "-F", "-"]);

    if let Some(author) = author {
        cmd.env("GIT_AUTHOR_NAME", &author.name);
        cmd.env("GIT_AUTHOR_EMAIL", &author.email);
        cmd.env("GIT_COMMITTER_NAME", &author.name);
        cmd.env("GIT_COMMITTER_EMAIL", &author.email);
    }

    run_command_checked("git commit", cmd, Some(message.as_bytes()))?;
    Ok(())
}

fn git_rev_parse_head(root: &Path) -> Result<String, GitError> {
    let mut cmd = git_command(root)?;
    cmd.args(["rev-parse", "HEAD"]);
    let output = run_command_checked("git rev-parse", cmd, None)?;
    let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if id.is_empty() {
        return Err(GitError::new("git rev-parse returned empty output"));
    }
    Ok(id)
}

fn git_has_head(root: &Path) -> Result<bool, GitError> {
    let mut cmd = git_command(root)?;
    cmd.args(["rev-parse", "--verify", "HEAD"]);
    let output = run_command("git rev-parse --verify", cmd, None)?;
    Ok(output.status.success())
}

fn git_diff_name_only(root: &Path) -> Result<usize, GitError> {
    let mut cmd = git_command(root)?;
    cmd.args([
        "diff",
        "--name-only",
        "--ignore-submodules=all",
        "--no-ext-diff",
        "--no-color",
        "--",
    ]);
    let output = run_command_checked("git diff --name-only", cmd, None)?;
    Ok(count_non_empty_lines(&output.stdout))
}

fn git_diff(root: &Path) -> Result<String, GitError> {
    let mut cmd = git_command(root)?;
    cmd.args([
        "diff",
        "--ignore-submodules=all",
        "--no-ext-diff",
        "--no-color",
        "--",
    ]);
    let output = run_command_checked("git diff", cmd, None)?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn git_command(root: &Path) -> Result<Command, GitError> {
    let root = std::fs::canonicalize(root)
        .map_err(|err| GitError::new(format!("canonicalize root: {err}")))?;
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(root);
    cmd.arg("-c")
        .arg(format!("core.hooksPath={}", null_device()));
    cmd.arg("-c").arg("credential.helper=");
    cmd.arg("-c").arg("commit.gpgsign=false");
    cmd.env("GIT_CONFIG_NOSYSTEM", "1");
    cmd.env("GIT_CONFIG_SYSTEM", null_device());
    cmd.env("GIT_CONFIG_GLOBAL", null_device());
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    Ok(cmd)
}

fn run_command(context: &str, mut cmd: Command, input: Option<&[u8]>) -> Result<Output, GitError> {
    let stdin = if input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    };
    let mut child = cmd
        .stdin(stdin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| GitError::new(format!("{context}: {err}")))?;

    if let Some(input) = input && let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(input)
            .map_err(|err| GitError::new(format!("{context}: {err}")))?;
    }

    child
        .wait_with_output()
        .map_err(|err| GitError::new(format!("{context}: {err}")))
}

fn run_command_checked(
    context: &str,
    cmd: Command,
    input: Option<&[u8]>,
) -> Result<Output, GitError> {
    let output = run_command(context, cmd, input)?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(GitError::new(format_git_error(context, &output)))
    }
}

fn format_git_error(context: &str, output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let message = if !stderr.trim().is_empty() {
        stderr.trim()
    } else if !stdout.trim().is_empty() {
        stdout.trim()
    } else {
        return format!("{context}: git exited with {}", output.status);
    };
    format!("{context}: {message}")
}

fn count_non_empty_lines(bytes: &[u8]) -> usize {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
}

fn has_non_empty_lines(bytes: &[u8]) -> bool {
    count_non_empty_lines(bytes) > 0
}

fn null_device() -> &'static str {
    if cfg!(windows) { "NUL" } else { "/dev/null" }
}

fn parse_gitdir_path(dot_git: &Path) -> std::io::Result<Option<PathBuf>> {
    let contents = std::fs::read_to_string(dot_git)?;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("gitdir:") {
            let path = rest.trim();
            if path.is_empty() {
                return Ok(None);
            }
            return Ok(Some(PathBuf::from(path)));
        }
    }
    Ok(None)
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::{GitAuthor, git_commit_all, git_dir_within_root, git_status_and_diff};
    use std::path::{Path, PathBuf};
    use std::process::Command;

    #[test]
    fn git_dir_within_root__should_accept_dot_git_directory() {
        // Given
        let root = create_temp_root("git-dir");
        let git_dir = root.join(".git");
        std::fs::create_dir_all(&git_dir).expect("create .git dir");

        // When
        let detected = git_dir_within_root(&root).expect("detect git dir");

        // Then
        let expected = std::fs::canonicalize(&git_dir).expect("canonicalize git dir");
        assert_eq!(detected, Some(expected));

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn git_dir_within_root__should_accept_gitdir_file_with_relative_path() {
        // Given
        let root = create_temp_root("gitdir-file");
        let actual_git = root.join("git-data");
        std::fs::create_dir_all(&actual_git).expect("create git dir");
        std::fs::write(root.join(".git"), "gitdir: git-data\n").expect("write .git file");

        // When
        let detected = git_dir_within_root(&root).expect("detect git dir");

        // Then
        let expected = std::fs::canonicalize(&actual_git).expect("canonicalize git dir");
        assert_eq!(detected, Some(expected));

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn git_dir_within_root__should_reject_gitdir_file_outside_root() {
        // Given
        let root = create_temp_root("gitdir-outside");
        let outside = create_temp_root("gitdir-outside-target");
        let outside_git = outside.join("repo");
        std::fs::create_dir_all(&outside_git).expect("create outside git dir");
        let contents = format!("gitdir: {}\n", outside_git.display());
        std::fs::write(root.join(".git"), contents).expect("write .git file");

        // When
        let detected = git_dir_within_root(&root).expect("detect git dir");

        // Then
        assert!(detected.is_none());

        std::fs::remove_dir_all(&root).expect("cleanup");
        std::fs::remove_dir_all(&outside).expect("cleanup outside");
    }

    #[test]
    fn git_dir_within_root__should_return_none_when_missing() {
        // Given
        let root = create_temp_root("git-missing");

        // When
        let detected = git_dir_within_root(&root).expect("detect git dir");

        // Then
        assert!(detected.is_none());

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn git_dir_within_root__should_ignore_invalid_gitdir_file() {
        // Given
        let root = create_temp_root("git-invalid");
        std::fs::write(root.join(".git"), "not-a-gitdir").expect("write .git file");

        // When
        let detected = git_dir_within_root(&root).expect("detect git dir");

        // Then
        assert!(detected.is_none());

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn git_dir_within_root__should_reject_symlink_outside_root() {
        // Given
        use std::os::unix::fs::symlink;

        let root = create_temp_root("git-symlink");
        let outside = create_temp_root("git-symlink-outside");
        let outside_git = outside.join(".git");
        std::fs::create_dir_all(&outside_git).expect("create outside git dir");
        symlink(&outside_git, root.join(".git")).expect("create .git symlink");

        // When
        let detected = git_dir_within_root(&root).expect("detect git dir");

        // Then
        assert!(detected.is_none());

        std::fs::remove_dir_all(&root).expect("cleanup");
        std::fs::remove_dir_all(&outside).expect("cleanup outside");
    }

    #[test]
    fn git_commit_all__should_commit_and_clear_status() {
        // Given
        let root = create_temp_root("git-commit");
        init_repo(&root);
        std::fs::write(root.join("note.md"), "Hello").expect("write note.md");
        let author = GitAuthor {
            name: "Marten".to_string(),
            email: "marten@example.com".to_string(),
        };

        // When
        let commit = git_commit_all(&root, "Initial commit", Some(author)).expect("commit");

        // Then
        let snapshot = git_status_and_diff(&root).expect("status");
        assert_eq!(snapshot.changed_files, 0);
        assert!(snapshot.diff.trim().is_empty());
        assert!(!commit.id.is_empty());

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    fn init_repo(root: &Path) {
        let status = Command::new("git")
            .arg("-C")
            .arg(root)
            .arg("init")
            .status()
            .expect("init repo");
        assert!(status.success());
    }

    fn create_temp_root(test_name: &str) -> PathBuf {
        let mut root = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        root.push(format!("mindex-{}-{}", test_name, nanos));
        std::fs::create_dir_all(&root).expect("create temp dir");
        root
    }
}
