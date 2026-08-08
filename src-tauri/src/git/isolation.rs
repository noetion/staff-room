use crate::*;

pub(crate) fn copy_workspace_file(source: &Path, destination: &Path) -> Result<(), String> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let metadata = std::fs::symlink_metadata(source).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() {
        let target = std::fs::read_link(source).map_err(|error| error.to_string())?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, destination).map_err(|error| error.to_string())?;
        #[cfg(windows)]
        {
            if source.is_dir() {
                std::os::windows::fs::symlink_dir(target, destination)
                    .map_err(|error| error.to_string())?;
            } else {
                std::os::windows::fs::symlink_file(target, destination)
                    .map_err(|error| error.to_string())?;
            }
        }
    } else {
        std::fs::copy(source, destination).map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(crate) fn remove_managed_clone(root: &Path, worktree: &Path) -> Result<(), String> {
    let root = std::fs::canonicalize(root).map_err(|error| error.to_string())?;
    let parent = worktree
        .parent()
        .ok_or_else(|| "Managed snapshot clone has no parent directory.".to_owned())?;
    let parent = std::fs::canonicalize(parent).map_err(|error| error.to_string())?;
    if parent != root || !worktree.join(".git").is_dir() {
        return Err(format!(
            "Refused to remove an unverified managed snapshot clone: {}",
            worktree.to_string_lossy()
        ));
    }
    std::fs::remove_dir_all(worktree).map_err(|error| error.to_string())
}

pub(crate) fn create_worktree(
    app: &AppHandle,
    repository: &Path,
    run_id: &str,
) -> Result<IsolationContext, String> {
    let root = app
        .path()
        .app_cache_dir()
        .map_err(|error| error.to_string())?
        .join("worktrees");
    create_isolation_at_root(repository, run_id, &root)
}

pub(crate) fn create_quick_edit_worktree(
    app: &AppHandle,
    repository: &Path,
    edit_id: &str,
) -> Result<PathBuf, String> {
    let root = app
        .path()
        .app_cache_dir()
        .map_err(|error| error.to_string())?
        .join("quick-edits");
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let short = edit_id
        .chars()
        .filter(|value| *value != '-')
        .take(10)
        .collect::<String>();
    let worktree = root.join(short);
    if worktree.exists() {
        return Err(format!(
            "Quick Edit worktree path already exists: {}",
            worktree.to_string_lossy()
        ));
    }
    git(
        repository,
        &[
            "worktree".to_owned(),
            "add".to_owned(),
            "--detach".to_owned(),
            worktree.to_string_lossy().into_owned(),
            "HEAD".to_owned(),
        ],
    )?;
    Ok(worktree)
}

pub(crate) fn remove_quick_edit_worktree(repository: &Path, worktree: &Path) -> Result<(), String> {
    git(
        repository,
        &[
            "worktree".to_owned(),
            "remove".to_owned(),
            "--force".to_owned(),
            worktree.to_string_lossy().into_owned(),
        ],
    )
    .map(|_| ())
}

pub(crate) fn quick_edit_diff(worktree: &Path) -> Result<String, String> {
    git_static(worktree, &["add", "-N", "--", "."])?;
    git_static(worktree, &["diff", "--binary", "--no-ext-diff", "HEAD"])
}

pub(crate) fn commit_managed_changes(worktree: &Path, objective: &str) -> Result<bool, String> {
    let status = git_static(worktree, &["status", "--porcelain"])?;
    if status.trim().is_empty() {
        return Ok(false);
    }
    git_static(worktree, &["add", "-A"])?;
    let subject = objective
        .lines()
        .next()
        .unwrap_or("Complete objective")
        .trim();
    let subject = truncate_utf8(subject, 72);
    git(
        worktree,
        &[
            "-c".to_owned(),
            "user.name=The Staff Room".to_owned(),
            "-c".to_owned(),
            "user.email=staff-room@local".to_owned(),
            "commit".to_owned(),
            "-m".to_owned(),
            format!("The Staff Room: {subject}"),
        ],
    )?;
    Ok(true)
}

pub(crate) fn diff_evidence(worktree: &Path, base_head: &str) -> String {
    // A swallowed error here used to produce "\n\n", which assemble_packet drops as
    // empty — so the reviewer silently received no delta at all and could still
    // answer "approved". Failure has to be loud enough that it cannot be approved
    // through.
    let diff = match git_static(
        worktree,
        &["diff", "--no-ext-diff", "--unified=2", base_head, "HEAD"],
    ) {
        Ok(diff) => diff,
        Err(error) => {
            return format!(
            "DELTA UNAVAILABLE: The Staff Room could not compute the repository delta ({error}). \
                 You have not been shown the change. Do not approve; \
                 return status `changes_required` citing missing evidence."
        )
        }
    };
    let stat = match git_static(worktree, &["diff", "--stat", base_head, "HEAD"]) {
        Ok(stat) => stat,
        Err(error) => format!("(diffstat unavailable: {error})"),
    };
    truncate_utf8(&format!("{stat}\n\n{diff}"), SOURCE_BUDGET_BYTES * 2)
}
