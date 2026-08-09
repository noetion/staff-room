mod commands;
mod db;
mod events;
mod git;
mod providers;
mod run;
mod types;

use commands::*;
use db::*;
use events::*;
use git::*;
use providers::*;
use run::*;
use types::*;

use providers::{Mode as ProviderMode, TurnRequest};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    ffi::OsStr,
    io::Read,
    path::{Path, PathBuf},
    process::Command as StdCommand,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tauri_plugin_notification::NotificationExt;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::{watch, Mutex as AsyncMutex},
    time::{sleep, timeout},
};
use uuid::Uuid;

#[cfg(windows)]
use std::os::windows::process::CommandExt as _;

fn workspace_fingerprint(repository: &Path) -> Result<String, String> {
    let status = git_bytes(
        repository,
        &[
            "status".to_owned(),
            "--porcelain=v1".to_owned(),
            "-z".to_owned(),
            "--untracked-files=all".to_owned(),
        ],
    )?;
    let mut paths = BTreeSet::new();
    for args in [
        &["ls-files", "-m", "-d", "-o", "--exclude-standard", "-z"][..],
        &["diff", "--cached", "--name-only", "-z"][..],
    ] {
        paths.extend(git_paths(repository, args)?);
    }

    let mut hasher = Sha256::new();
    hasher.update(&status);
    for relative in paths {
        hasher.update([0]);
        hasher.update(relative.as_bytes());
        let path = repository.join(&relative);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                hasher.update(b"symlink");
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    hasher.update(metadata.mode().to_le_bytes());
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    hasher.update(metadata.file_attributes().to_le_bytes());
                }
                hasher.update(
                    std::fs::read_link(&path)
                        .map_err(|error| format!("Failed to inspect {relative}: {error}"))?
                        .to_string_lossy()
                        .as_bytes(),
                );
            }
            Ok(metadata) if metadata.is_file() => {
                hasher.update(b"file");
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    hasher.update(metadata.mode().to_le_bytes());
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    hasher.update(metadata.file_attributes().to_le_bytes());
                }
                let mut file = std::fs::File::open(&path)
                    .map_err(|error| format!("Failed to inspect {relative}: {error}"))?;
                let mut buffer = [0_u8; 64 * 1024];
                loop {
                    let count = file
                        .read(&mut buffer)
                        .map_err(|error| format!("Failed to inspect {relative}: {error}"))?;
                    if count == 0 {
                        break;
                    }
                    hasher.update(&buffer[..count]);
                }
            }
            Ok(_) => hasher.update(b"directory"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => hasher.update(b"missing"),
            Err(error) => return Err(format!("Failed to inspect {relative}: {error}")),
        }
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn create_isolation_at_root(
    repository: &Path,
    run_id: &str,
    root: &Path,
) -> Result<IsolationContext, String> {
    let base_changes = git_static(repository, &["status", "--porcelain"])?;
    let base_head = git_static(repository, &["rev-parse", "HEAD"])?;
    let base_branch = git_static(repository, &["branch", "--show-current"])?;
    if base_branch.trim().is_empty() {
        return Err("The Staff Room requires an attached branch, not a detached HEAD.".to_owned());
    }
    let short = run_id
        .chars()
        .filter(|value| *value != '-')
        .take(10)
        .collect::<String>();
    let branch = format!("staff-room/{short}");
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    let worktree = root.join(&short);
    if worktree.exists() {
        return Err(format!(
            "Managed worktree path already exists: {}",
            worktree.to_string_lossy()
        ));
    }

    if base_changes.trim().is_empty() {
        git(
            repository,
            &[
                "worktree".to_owned(),
                "add".to_owned(),
                "-b".to_owned(),
                branch.clone(),
                worktree.to_string_lossy().into_owned(),
                base_head.clone(),
            ],
        )?;
        return Ok(IsolationContext {
            worktree,
            branch,
            base_branch,
            snapshot_head: base_head.clone(),
            base_head,
            isolation_kind: "worktree".to_owned(),
            workspace_fingerprint: None,
        });
    }

    let fingerprint = workspace_fingerprint(repository)?;
    let patch = root.join(format!(".snapshot-{short}.patch"));
    let snapshot_result = (|| {
        git(
            repository,
            &[
                "diff".to_owned(),
                "--binary".to_owned(),
                "--no-ext-diff".to_owned(),
                "HEAD".to_owned(),
                format!("--output={}", patch.to_string_lossy()),
            ],
        )?;
        git(
            repository,
            &[
                "clone".to_owned(),
                "--no-checkout".to_owned(),
                repository.to_string_lossy().into_owned(),
                worktree.to_string_lossy().into_owned(),
            ],
        )?;
        git(
            &worktree,
            &[
                "checkout".to_owned(),
                "-b".to_owned(),
                branch.clone(),
                base_head.clone(),
            ],
        )?;
        // This is the only place in this closure that turned an error into a
        // no-op. A stat failure (sharing violation from an AV scanner, a
        // permission or long-path failure on the managed root) used to read as
        // "empty patch", so the snapshot silently omitted every uncommitted
        // change to tracked files and the agent built against a stale premise
        // that no later guard catches. A missing file is still treated as empty,
        // because git omits the output file when there is nothing to diff.
        let patch_bytes = match patch.metadata() {
            Ok(value) => value.len(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
            Err(error) => {
                return Err(format!(
                    "Could not inspect the workspace snapshot patch: {error}"
                ))
            }
        };
        if patch_bytes > 0 {
            git(
                &worktree,
                &[
                    "apply".to_owned(),
                    "--binary".to_owned(),
                    patch.to_string_lossy().into_owned(),
                ],
            )?;
        }
        for relative in git_paths(
            repository,
            &["ls-files", "--others", "--exclude-standard", "-z"],
        )? {
            copy_workspace_file(&repository.join(&relative), &worktree.join(&relative))?;
        }
        if !commit_managed_changes(&worktree, "Capture current workspace state")? {
            return Err(
                "Ship cannot safely snapshot these uncommitted changes. Nested repository or submodule changes must be committed or stashed first."
                    .to_owned(),
            );
        }
        let snapshot_head = git_static(&worktree, &["rev-parse", "HEAD"])?;
        Ok(IsolationContext {
            worktree: worktree.clone(),
            branch,
            base_branch,
            base_head,
            snapshot_head,
            isolation_kind: "snapshot-clone".to_owned(),
            workspace_fingerprint: Some(fingerprint),
        })
    })();
    let _ = std::fs::remove_file(&patch);
    if snapshot_result.is_err() && worktree.exists() {
        let _ = remove_managed_clone(root, &worktree);
    }
    snapshot_result
}

fn promote_worktree(
    base_repository: &Path,
    isolation: &IsolationContext,
    managed_root: &Path,
) -> Result<PromotionResult, String> {
    let worktree = &isolation.worktree;
    let branch = &isolation.branch;
    let base_head = &isolation.base_head;
    let snapshot_head = &isolation.snapshot_head;
    let current_branch = git_static(base_repository, &["branch", "--show-current"])?;
    if current_branch != isolation.base_branch {
        return Err(format!(
            "The attached checkout moved from branch `{}` to {} while Ship was active. The verified work remains isolated.",
            isolation.base_branch,
            if current_branch.is_empty() {
                "a detached HEAD".to_owned()
            } else {
                format!("branch `{current_branch}`")
            }
        ));
    }
    let current_head = git_static(base_repository, &["rev-parse", "HEAD"])?;
    if current_head != base_head.as_str() {
        return Err("The base branch advanced while the run was active.".to_owned());
    }

    match isolation.isolation_kind.as_str() {
        "worktree" => {
            let base_status = git_static(base_repository, &["status", "--porcelain"])?;
            if !base_status.trim().is_empty() {
                return Err("The base checkout changed while the run was active.".to_owned());
            }
            git(
                base_repository,
                &[
                    "merge".to_owned(),
                    "--ff-only".to_owned(),
                    branch.to_owned(),
                ],
            )?;
            let mut cleanup_errors = Vec::new();
            if let Err(error) = git(
                base_repository,
                &[
                    "worktree".to_owned(),
                    "remove".to_owned(),
                    "--force".to_owned(),
                    worktree.to_string_lossy().into_owned(),
                ],
            ) {
                cleanup_errors.push(error);
            }
            if let Err(error) = git(
                base_repository,
                &["branch".to_owned(), "-d".to_owned(), branch.to_owned()],
            ) {
                cleanup_errors.push(error);
            }
            Ok(PromotionResult {
                mode: PromotionMode::FastForward,
                cleanup_warning: (!cleanup_errors.is_empty()).then(|| cleanup_errors.join("\n")),
            })
        }
        "snapshot-clone" => {
            let expected = isolation.workspace_fingerprint.as_deref().ok_or_else(|| {
                "The preserved workspace snapshot is missing its safety fingerprint.".to_owned()
            })?;
            if workspace_fingerprint(base_repository)? != expected {
                return Err(
                    "The attached checkout changed while Ship was active. The verified work remains isolated so your newer edits are not overwritten."
                        .to_owned(),
                );
            }
            let patch = managed_root.join(format!(".promote-{}.patch", Uuid::new_v4()));
            let apply_result = (|| {
                git(
                    worktree,
                    &[
                        "diff".to_owned(),
                        "--binary".to_owned(),
                        "--no-ext-diff".to_owned(),
                        snapshot_head.to_owned(),
                        "HEAD".to_owned(),
                        format!("--output={}", patch.to_string_lossy()),
                    ],
                )?;
                git(
                    base_repository,
                    &[
                        "apply".to_owned(),
                        "--check".to_owned(),
                        "--binary".to_owned(),
                        patch.to_string_lossy().into_owned(),
                    ],
                )?;
                git(
                    base_repository,
                    &[
                        "apply".to_owned(),
                        "--binary".to_owned(),
                        patch.to_string_lossy().into_owned(),
                    ],
                )
            })();
            let _ = std::fs::remove_file(&patch);
            apply_result?;
            let cleanup_warning = remove_managed_clone(managed_root, worktree).err();
            Ok(PromotionResult {
                mode: PromotionMode::WorkingTree,
                cleanup_warning,
            })
        }
        other => Err(format!("Unknown Ship isolation kind: {other}")),
    }
}

fn discard_isolation(
    base_repository: &Path,
    isolation: &IsolationContext,
    managed_root: &Path,
) -> Option<String> {
    if isolation.isolation_kind == "snapshot-clone" {
        return remove_managed_clone(managed_root, &isolation.worktree).err();
    }
    let mut errors = Vec::new();
    if let Err(error) = git(
        base_repository,
        &[
            "worktree".to_owned(),
            "remove".to_owned(),
            "--force".to_owned(),
            isolation.worktree.to_string_lossy().into_owned(),
        ],
    ) {
        errors.push(error);
    }
    if let Err(error) = git(
        base_repository,
        &[
            "branch".to_owned(),
            "-D".to_owned(),
            isolation.branch.clone(),
        ],
    ) {
        errors.push(error);
    }
    (!errors.is_empty()).then(|| errors.join("\n"))
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_directory = app.path().app_data_dir()?;
            let local_data_directory = app.path().app_local_data_dir()?;
            let database_path = prepare_app_data(&data_directory, &local_data_directory)
                .map_err(std::io::Error::other)?;
            let connection = Connection::open(database_path)?;
            migrate(&connection)?;
            reconcile_interrupted_runs(&connection)?;
            app.manage(Database(Mutex::new(connection)));
            app.manage(RuntimeState::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_environment,
            project_pick,
            project_attach,
            project_list,
            project_select,
            project_active,
            allocate_operation_id,
            load_project_settings,
            save_project_settings,
            load_verification_config,
            save_verification_config,
            load_provider_profiles,
            save_provider_profile,
            load_room,
            test_provider_connection,
            discover_provider_models,
            start_room_chat,
            quick_edit_start,
            quick_edit_apply,
            quick_edit_discard,
            start_room_run,
            approve_run_promotion,
            stop_run,
            abandon_run,
            voice_status,
            voice_pick_model,
            voice_pick_engine,
            voice_start,
            voice_stop
        ])
        .run(tauri::generate_context!())
        .expect("error while running The Staff Room");
}

#[cfg(test)]
mod tests;
