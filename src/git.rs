use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use gix::bstr::{BStr, BString, ByteSlice};
use gix::status::UntrackedFiles;
use gix::status::index_worktree::Item as WorktreeItem;
use gix::status::plumbing::index_as_worktree::{Change as WorktreeChange, EntryStatus};
use imara_diff::intern::InternedInput;
use imara_diff::{Algorithm, UnifiedDiffBuilder, diff};

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
    pub(crate) id: gix::hash::ObjectId,
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
    let repo = gix::open(root).map_err(|err| GitError::new(format!("open repo: {err}")))?;
    if repo.head_commit().is_err() {
        return Ok(GitSnapshot {
            changed_files: 0,
            diff: String::new(),
        });
    }
    let index = open_or_create_index(&repo)?;
    let iter = repo
        .status(gix::progress::Discard)
        .map_err(|err| GitError::new(format!("status: {err}")))?
        .index(index.into())
        .index_worktree_submodules(None)
        .untracked_files(gix::status::UntrackedFiles::None)
        .into_index_worktree_iter(Vec::new())
        .map_err(|err| GitError::new(format!("status iterator: {err}")))?;

    let mut diff = String::new();
    let mut changed_files = 0usize;
    for item in iter {
        let item = item.map_err(|err| GitError::new(format!("status item: {err}")))?;
        if item.summary().is_some() {
            changed_files += 1;
        }
        let Some(file_diff) = diff_for_item(&repo, root, &item)? else {
            continue;
        };
        diff.push_str(&file_diff);
    }

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
    let repo = gix::open(&root).map_err(|err| GitError::new(format!("open repo: {err}")))?;
    let mut index = open_or_create_index(&repo)?;
    ensure_unconflicted_index(&index)?;
    stage_worktree_changes(&repo, &root, &mut index)?;
    index.sort_entries();

    let tree_id = write_tree_from_index(&repo, &index)?;
    let head_tree = repo.head_tree_id().ok().map(|id| id.detach());
    let no_changes = match head_tree {
        Some(head_tree) => head_tree == tree_id,
        None => index.entries().is_empty(),
    };
    if no_changes {
        return Err(GitError::new("no changes to commit"));
    }

    index
        .write(gix::index::write::Options::default())
        .map_err(|err| GitError::new(format!("write index: {err}")))?;

    let parents = match repo.head_commit() {
        Ok(commit) => vec![commit.id],
        Err(_) => Vec::new(),
    };

    let commit_id = match author {
        Some(author) => {
            let time = gix::date::Time::now_local_or_utc().to_string();
            let signature = gix::actor::SignatureRef {
                name: author.name.as_str().into(),
                email: author.email.as_str().into(),
                time: time.as_str(),
            };
            repo.commit_as(signature, signature, "HEAD", message, tree_id, parents)
        }
        None => {
            let committer = repo
                .committer()
                .ok_or_else(|| GitError::new("git committer is not configured"))?
                .map_err(|err| GitError::new(format!("committer time: {err}")))?;
            let author = repo
                .author()
                .ok_or_else(|| GitError::new("git author is not configured"))?
                .map_err(|err| GitError::new(format!("author time: {err}")))?;
            repo.commit_as(committer, author, "HEAD", message, tree_id, parents)
        }
    }
    .map_err(|err| GitError::new(format!("commit: {err}")))?;

    Ok(GitCommit {
        id: commit_id.detach(),
    })
}

fn open_or_create_index(repo: &gix::Repository) -> Result<gix::index::File, GitError> {
    match repo.open_index() {
        Ok(index) => Ok(index),
        Err(err) => match err {
            gix::worktree::open_index::Error::IndexFile(gix::index::file::init::Error::Io(err))
                if err.kind() == std::io::ErrorKind::NotFound =>
            {
                Ok(gix::index::File::from_state(
                    gix::index::State::new(repo.object_hash()),
                    repo.index_path(),
                ))
            }
            err => Err(GitError::new(format!("open index: {err}"))),
        },
    }
}

fn ensure_unconflicted_index(index: &gix::index::File) -> Result<(), GitError> {
    if index
        .entries()
        .iter()
        .any(|entry| entry.stage() != gix::index::entry::Stage::Unconflicted)
    {
        return Err(GitError::new("conflicted index entries are not supported"));
    }
    Ok(())
}

#[derive(Clone)]
struct StagedEntry {
    id: gix::hash::ObjectId,
    stat: gix::index::entry::Stat,
    mode: gix::index::entry::Mode,
}

struct WorktreeFile {
    data: Vec<u8>,
    stat: gix::index::entry::Stat,
    mode: gix::index::entry::Mode,
}

fn stage_worktree_changes(
    repo: &gix::Repository,
    root: &Path,
    index: &mut gix::index::File,
) -> Result<(), GitError> {
    if repo.head_commit().is_err() {
        return stage_all_files(repo, root, index);
    }

    let iter = repo
        .status(gix::progress::Discard)
        .map_err(|err| GitError::new(format!("status: {err}")))?
        .index(index.clone().into())
        .index_worktree_submodules(None)
        .untracked_files(UntrackedFiles::Files)
        .into_index_worktree_iter(Vec::new())
        .map_err(|err| GitError::new(format!("status iterator: {err}")))?;

    let mut upserts: HashMap<BString, StagedEntry> = HashMap::new();
    let mut removals: HashSet<BString> = HashSet::new();
    let mut stat_updates: HashMap<BString, gix::index::entry::Stat> = HashMap::new();

    for item in iter {
        let item = item.map_err(|err| GitError::new(format!("status item: {err}")))?;
        match item {
            WorktreeItem::Modification {
                rela_path, status, ..
            } => {
                let path: &BStr = rela_path.as_ref();
                match status {
                    EntryStatus::Conflict { .. } => {
                        return Err(GitError::new("conflicts are not supported"));
                    }
                    EntryStatus::NeedsUpdate(stat) => {
                        stat_updates.insert(path.to_owned(), stat);
                    }
                    EntryStatus::IntentToAdd => {
                        let staged = stage_from_path(repo, root, path)?.ok_or_else(|| {
                            GitError::new(format!(
                                "unable to stage intent-to-add file '{}'",
                                path.to_str_lossy()
                            ))
                        })?;
                        upserts.insert(path.to_owned(), staged);
                        removals.remove(path);
                    }
                    EntryStatus::Change(change) => match change {
                        WorktreeChange::Removed => {
                            upserts.remove(path);
                            removals.insert(path.to_owned());
                        }
                        WorktreeChange::SubmoduleModification(_) => {
                            return Err(GitError::new("submodule changes are not supported"));
                        }
                        WorktreeChange::Modification { .. } => {
                            let staged = stage_from_path(repo, root, path)?.ok_or_else(|| {
                                GitError::new(format!(
                                    "unable to stage file '{}'",
                                    path.to_str_lossy()
                                ))
                            })?;
                            upserts.insert(path.to_owned(), staged);
                            removals.remove(path);
                        }
                        WorktreeChange::Type { .. } => match stage_from_path(repo, root, path)? {
                            Some(staged) => {
                                upserts.insert(path.to_owned(), staged);
                                removals.remove(path);
                            }
                            None => {
                                upserts.remove(path);
                                removals.insert(path.to_owned());
                            }
                        },
                    },
                }
            }
            WorktreeItem::DirectoryContents { entry, .. } => {
                if entry.status != gix::dir::entry::Status::Untracked {
                    continue;
                }
                if matches!(
                    entry.disk_kind,
                    Some(gix::dir::entry::Kind::Directory)
                        | Some(gix::dir::entry::Kind::Repository)
                ) {
                    continue;
                }
                let path: &BStr = entry.rela_path.as_ref();
                let staged = match stage_from_path(repo, root, path)? {
                    Some(staged) => staged,
                    None => continue,
                };
                upserts.insert(path.to_owned(), staged);
                removals.remove(path);
            }
            WorktreeItem::Rewrite {
                source,
                dirwalk_entry,
                copy,
                ..
            } => {
                let dest_path: &BStr = dirwalk_entry.rela_path.as_ref();
                if !copy {
                    let source_path = source.rela_path();
                    upserts.remove(source_path);
                    removals.insert(source_path.to_owned());
                }
                if matches!(
                    dirwalk_entry.disk_kind,
                    Some(gix::dir::entry::Kind::Directory)
                        | Some(gix::dir::entry::Kind::Repository)
                ) {
                    continue;
                }
                let staged = match stage_from_path(repo, root, dest_path)? {
                    Some(staged) => staged,
                    None => continue,
                };
                upserts.insert(dest_path.to_owned(), staged);
                removals.remove(dest_path);
            }
        }
    }

    apply_stage_updates(index, removals, upserts, stat_updates);
    Ok(())
}

fn stage_all_files(
    repo: &gix::Repository,
    root: &Path,
    index: &mut gix::index::File,
) -> Result<(), GitError> {
    let mut upserts: HashMap<BString, StagedEntry> = HashMap::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries =
            std::fs::read_dir(&dir).map_err(|err| GitError::new(format!("read dir: {err}")))?;
        for entry in entries {
            let entry = entry.map_err(|err| GitError::new(format!("read dir entry: {err}")))?;
            let file_type = entry
                .file_type()
                .map_err(|err| GitError::new(format!("read dir entry type: {err}")))?;
            let path = entry.path();

            if file_type.is_dir() {
                if entry.file_name() == ".git" {
                    continue;
                }
                stack.push(path);
                continue;
            }

            if file_type.is_symlink() {
                return Err(GitError::new(format!(
                    "symlinks are not supported: {}",
                    path.display()
                )));
            }

            if !file_type.is_file() {
                continue;
            }

            let rel = path
                .strip_prefix(root)
                .map_err(|_| GitError::new("path escaped root"))?;
            let rel = rel.to_string_lossy().replace('\\', "/");
            if rel.is_empty() {
                continue;
            }
            let rel_bstring = BString::from(rel);
            let staged = stage_from_path(repo, root, rel_bstring.as_ref())?.ok_or_else(|| {
                GitError::new(format!(
                    "unable to stage file '{}'",
                    rel_bstring.to_str_lossy()
                ))
            })?;
            upserts.insert(rel_bstring, staged);
        }
    }

    apply_stage_updates(index, HashSet::new(), upserts, HashMap::new());
    Ok(())
}

fn apply_stage_updates(
    index: &mut gix::index::File,
    removals: HashSet<BString>,
    mut upserts: HashMap<BString, StagedEntry>,
    mut stat_updates: HashMap<BString, gix::index::entry::Stat>,
) {
    if !removals.is_empty() {
        index.remove_entries(|_, path, _| removals.contains(path));
    }

    for (entry, path) in index.entries_mut_with_paths() {
        if let Some(update) = upserts.remove(path) {
            entry.id = update.id;
            entry.stat = update.stat;
            entry.mode = update.mode;
            entry
                .flags
                .remove(gix::index::entry::Flags::REMOVE | gix::index::entry::Flags::INTENT_TO_ADD);
        }
        if let Some(stat) = stat_updates.remove(path) {
            entry.stat = stat;
        }
    }

    for (path, update) in upserts {
        index.dangerously_push_entry(
            update.stat,
            update.id,
            gix::index::entry::Flags::empty(),
            update.mode,
            path.as_ref(),
        );
    }
}

fn stage_from_path(
    repo: &gix::Repository,
    root: &Path,
    rela_path: &BStr,
) -> Result<Option<StagedEntry>, GitError> {
    let Some(file) = read_worktree_file(root, rela_path)? else {
        return Ok(None);
    };
    let id = repo
        .write_blob(&file.data)
        .map_err(|err| GitError::new(format!("write blob: {err}")))?
        .detach();
    Ok(Some(StagedEntry {
        id,
        stat: file.stat,
        mode: file.mode,
    }))
}

fn read_worktree_file(root: &Path, rela_path: &BStr) -> Result<Option<WorktreeFile>, GitError> {
    let rel = rela_path.to_str_lossy();
    let path = root.join(rel.as_ref());
    let metadata = match gix::index::fs::Metadata::from_path_no_follow(&path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(GitError::new(format!("read worktree metadata: {err}"))),
    };
    if metadata.is_dir() {
        return Ok(None);
    }
    if metadata.is_symlink() {
        return Err(GitError::new(format!(
            "symlinks are not supported: {}",
            rel
        )));
    }
    if !metadata.is_file() {
        return Err(GitError::new(format!("unsupported file type: {}", rel)));
    }

    let resolved = match std::fs::canonicalize(&path) {
        Ok(path) => path,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(GitError::new(format!("read worktree: {err}"))),
    };
    if !resolved.starts_with(root) {
        return Err(GitError::new("worktree path escapes root"));
    }
    let data = match std::fs::read(&resolved) {
        Ok(data) => data,
        Err(err) if err.kind() == std::io::ErrorKind::IsADirectory => return Ok(None),
        Err(err) => return Err(GitError::new(format!("read worktree file: {err}"))),
    };
    let stat = gix::index::entry::Stat::from_fs(&metadata)
        .map_err(|err| GitError::new(format!("read worktree stat: {err}")))?;
    let mode = if metadata.is_executable() {
        gix::index::entry::Mode::FILE_EXECUTABLE
    } else {
        gix::index::entry::Mode::FILE
    };
    Ok(Some(WorktreeFile { data, stat, mode }))
}

#[derive(Default)]
struct TreeNode {
    entries: BTreeMap<BString, TreeNodeEntry>,
}

enum TreeNodeEntry {
    File {
        oid: gix::hash::ObjectId,
        mode: gix::index::entry::Mode,
    },
    Dir(TreeNode),
}

fn write_tree_from_index(
    repo: &gix::Repository,
    index: &gix::index::File,
) -> Result<gix::hash::ObjectId, GitError> {
    let mut root = TreeNode::default();
    let backing = index.path_backing();
    for entry in index.entries() {
        if entry.flags.contains(gix::index::entry::Flags::REMOVE) {
            continue;
        }
        if entry.stage() != gix::index::entry::Stage::Unconflicted {
            return Err(GitError::new("conflicted index entries are not supported"));
        }
        if entry.mode == gix::index::entry::Mode::DIR {
            return Err(GitError::new("sparse checkout entries are not supported"));
        }
        let path = entry.path_in(backing);
        insert_tree_entry(&mut root, path, entry.id, entry.mode)?;
    }

    write_tree_node(repo, &root)
}

fn insert_tree_entry(
    root: &mut TreeNode,
    path: &BStr,
    oid: gix::hash::ObjectId,
    mode: gix::index::entry::Mode,
) -> Result<(), GitError> {
    let components: Vec<&[u8]> = path
        .split_str("/")
        .filter(|part| !part.is_empty())
        .collect();
    if components.is_empty() {
        return Ok(());
    }

    let mut current = root;
    for (idx, component) in components.iter().enumerate() {
        if *component == b"." || *component == b".." {
            return Err(GitError::new(format!(
                "invalid path component in '{}'",
                path.to_str_lossy()
            )));
        }
        let name = BString::from(*component);
        let is_last = idx + 1 == components.len();
        if is_last {
            if let Some(TreeNodeEntry::Dir(_)) = current.entries.get(&name) {
                return Err(GitError::new(format!(
                    "path conflict at '{}'",
                    path.to_str_lossy()
                )));
            }
            current
                .entries
                .insert(name, TreeNodeEntry::File { oid, mode });
        } else {
            let entry = current
                .entries
                .entry(name)
                .or_insert_with(|| TreeNodeEntry::Dir(TreeNode::default()));
            match entry {
                TreeNodeEntry::Dir(child) => {
                    current = child;
                }
                TreeNodeEntry::File { .. } => {
                    return Err(GitError::new(format!(
                        "path conflict at '{}'",
                        path.to_str_lossy()
                    )));
                }
            }
        }
    }
    Ok(())
}

fn write_tree_node(
    repo: &gix::Repository,
    node: &TreeNode,
) -> Result<gix::hash::ObjectId, GitError> {
    let mut entries = Vec::new();
    for (name, entry) in &node.entries {
        match entry {
            TreeNodeEntry::File { oid, mode } => {
                let mode = mode
                    .to_tree_entry_mode()
                    .ok_or_else(|| GitError::new("invalid tree entry mode"))?;
                entries.push(gix::objs::tree::Entry {
                    mode,
                    filename: name.clone(),
                    oid: *oid,
                });
            }
            TreeNodeEntry::Dir(child) => {
                let child_id = write_tree_node(repo, child)?;
                entries.push(gix::objs::tree::Entry {
                    mode: gix::objs::tree::EntryMode::from(gix::objs::tree::EntryKind::Tree),
                    filename: name.clone(),
                    oid: child_id,
                });
            }
        }
    }
    entries.sort();
    let tree = gix::objs::Tree { entries };
    let id = repo
        .write_object(tree)
        .map_err(|err| GitError::new(format!("write tree: {err}")))?;
    Ok(id.detach())
}

fn diff_for_item(
    repo: &gix::Repository,
    root: &Path,
    item: &WorktreeItem,
) -> Result<Option<String>, GitError> {
    let (entry, rela_path, status) = match item {
        WorktreeItem::Modification {
            entry,
            rela_path,
            status,
            ..
        } => (entry, rela_path.as_ref(), status),
        _ => return Ok(None),
    };

    if entry.mode == gix::index::entry::Mode::COMMIT {
        return Ok(None);
    }

    let (old_bytes, new_bytes) = match status {
        EntryStatus::Change(change) => match change {
            WorktreeChange::Removed => {
                let old = blob_bytes(repo, entry.id)?;
                (Some(old), None)
            }
            WorktreeChange::Modification { .. } | WorktreeChange::Type { .. } => {
                let old = blob_bytes(repo, entry.id)?;
                let new = read_worktree_bytes(root, rela_path)?;
                (Some(old), new)
            }
            WorktreeChange::SubmoduleModification(_) => return Ok(None),
        },
        _ => return Ok(None),
    };

    let path = rela_path.to_str_lossy();
    Ok(Some(build_file_diff(
        &path,
        old_bytes.as_deref(),
        new_bytes.as_deref(),
    )))
}

fn blob_bytes(repo: &gix::Repository, id: gix::hash::ObjectId) -> Result<Vec<u8>, GitError> {
    let mut blob = repo
        .find_blob(id)
        .map_err(|err| GitError::new(format!("load blob: {err}")))?;
    Ok(blob.take_data())
}

fn read_worktree_bytes(root: &Path, rela_path: &BStr) -> Result<Option<Vec<u8>>, GitError> {
    let rel = rela_path.to_str_lossy();
    let path = root.join(rel.as_ref());
    let resolved = match std::fs::canonicalize(&path) {
        Ok(path) => path,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(GitError::new(format!("read worktree: {err}"))),
    };
    if !resolved.starts_with(root) {
        return Ok(None);
    }
    let data = match std::fs::read(&resolved) {
        Ok(data) => data,
        Err(err) if err.kind() == std::io::ErrorKind::IsADirectory => return Ok(None),
        Err(err) => return Err(GitError::new(format!("read worktree file: {err}"))),
    };
    Ok(Some(data))
}

fn build_file_diff(path: &str, old: Option<&[u8]>, new: Option<&[u8]>) -> String {
    let mut output = String::new();
    output.push_str(&format!("diff --git a/{path} b/{path}\n"));

    match (old, new) {
        (None, Some(_)) => {
            output.push_str("--- /dev/null\n");
            output.push_str(&format!("+++ b/{path}\n"));
        }
        (Some(_), None) => {
            output.push_str(&format!("--- a/{path}\n"));
            output.push_str("+++ /dev/null\n");
        }
        (Some(_), Some(_)) => {
            output.push_str(&format!("--- a/{path}\n"));
            output.push_str(&format!("+++ b/{path}\n"));
        }
        (None, None) => return output,
    }

    if old.map(is_binary).unwrap_or(false) || new.map(is_binary).unwrap_or(false) {
        output.push_str(&format!("Binary files a/{path} and b/{path} differ\n"));
        return output;
    }

    let before = old
        .map(String::from_utf8_lossy)
        .unwrap_or_else(|| "".into());
    let after = new
        .map(String::from_utf8_lossy)
        .unwrap_or_else(|| "".into());

    let input = InternedInput::new(before.as_ref(), after.as_ref());
    let mut file_diff = diff(
        Algorithm::Histogram,
        &input,
        UnifiedDiffBuilder::new(&input),
    );
    if !file_diff.ends_with('\n') && !file_diff.is_empty() {
        file_diff.push('\n');
    }
    output.push_str(&file_diff);
    output
}

fn is_binary(bytes: &[u8]) -> bool {
    bytes.contains(&0)
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
    use std::path::PathBuf;

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
        gix::init(&root).expect("init repo");
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
        assert!(!commit.id.to_string().is_empty());

        std::fs::remove_dir_all(&root).expect("cleanup");
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
