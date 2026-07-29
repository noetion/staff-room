use crate::*;

pub(crate) async fn run_verification(
    app: &AppHandle,
    database: &Database,
    project_id: &str,
    run_id: &str,
    repository: &Path,
    mut cancellation: watch::Receiver<bool>,
) -> Vec<VerificationResult> {
    let config = match verification_config(database, project_id) {
        Ok(config) => config,
        Err(error) => {
            return vec![VerificationResult {
                label: "Project checks".to_owned(),
                status: "unavailable".to_owned(),
                detail: format!("Verification configuration could not be loaded: {error}"),
            }]
        }
    };
    if !config.enabled {
        return vec![VerificationResult {
            label: "Project checks".to_owned(),
            status: "not-run".to_owned(),
            detail: "Verification is disabled for this project.".to_owned(),
        }];
    }
    let commands = config
        .commands
        .into_iter()
        .filter(|command| command.enabled)
        .collect::<Vec<_>>();
    if commands.is_empty() {
        return vec![VerificationResult {
            label: "Project checks".to_owned(),
            status: "not-run".to_owned(),
            detail: "No verification is configured for this project.".to_owned(),
        }];
    }

    let mut results = Vec::new();
    if let Some(prepare) = config.prepare {
        emit_event(
            app,
            run_id,
            "phase",
            "verify",
            "preparing",
            None,
            "Preparing verification dependencies",
            "Verification preparation is executed in the managed worktree.",
            None,
        );
        let prepare_result = run_verification_command(
            "Prepare dependencies",
            &prepare,
            repository,
            &mut cancellation,
            "unavailable",
        )
        .await;
        let prepare_succeeded = prepare_result.status == "passed";
        results.push(prepare_result);
        if !prepare_succeeded || *cancellation.borrow() {
            return results;
        }
    }
    for command in commands {
        emit_event(
            app,
            run_id,
            "phase",
            "verify",
            "verifying",
            None,
            &format!("Running {}", command.label),
            "Verification is executed directly in the managed worktree.",
            None,
        );
        results.push(
            run_verification_command(
                &command.label,
                &command.command,
                repository,
                &mut cancellation,
                "failed",
            )
            .await,
        );
        if *cancellation.borrow() {
            break;
        }
    }
    results
}

pub(crate) async fn run_verification_command(
    label: &str,
    command_text: &str,
    repository: &Path,
    cancellation: &mut watch::Receiver<bool>,
    failure_status: &str,
) -> VerificationResult {
    if let Some(tool) = command_text.split_whitespace().next() {
        let candidates = match tool {
            "npm" => vec!["npm", "npm.cmd"],
            "cargo" => vec!["cargo"],
            _ => Vec::new(),
        };
        if !candidates.is_empty() && find_executable(&candidates).is_none() {
            return VerificationResult {
                label: label.to_owned(),
                status: "unavailable".to_owned(),
                detail: format!("Required verification tool `{tool}` is not available."),
            };
        }
    }
    #[cfg(windows)]
    let mut process = {
        let mut process = Command::new("cmd");
        process.args(["/C", command_text]);
        process
    };
    #[cfg(not(windows))]
    let mut process = {
        let mut process = Command::new("sh");
        process.args(["-lc", command_text]);
        process
    };
    apply_provider_environment(&mut process, repository);
    process
        .current_dir(repository)
        .kill_on_drop(true)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    match process.spawn() {
        Ok(child) => {
            // wait_with_output consumes the child, so on cancel/timeout only
            // kill_on_drop runs — and on Windows that kills cmd.exe while the
            // real test runner keeps executing inside the managed worktree,
            // holding handles that later break worktree removal.
            let child_pid = child.id();
            let outcome = tokio::select! {
                output = child.wait_with_output() => output.ok(),
                _ = cancellation.changed() => None,
                _ = sleep(Duration::from_secs(10 * 60)) => None,
            };
            if outcome.is_none() {
                kill_pid_tree(child_pid).await;
            }
            match outcome {
                Some(output) => {
                    let combined = format!(
                        "{}\n{}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    );
                    VerificationResult {
                        label: label.to_owned(),
                        status: if output.status.success() {
                            "passed"
                        } else {
                            failure_status
                        }
                        .to_owned(),
                        detail: truncate_utf8(combined.trim(), 3 * 1024),
                    }
                }
                None => VerificationResult {
                    label: label.to_owned(),
                    status: if *cancellation.borrow() {
                        "not-run"
                    } else {
                        failure_status
                    }
                    .to_owned(),
                    detail: if *cancellation.borrow() {
                        "Verification stopped with the run."
                    } else {
                        "Verification exceeded the 10 minute limit."
                    }
                    .to_owned(),
                },
            }
        }
        Err(error) => VerificationResult {
            label: label.to_owned(),
            status: "unavailable".to_owned(),
            detail: format!("Failed to start verification: {error}"),
        },
    }
}

/// Every configured check must have actually completed and passed.
///
/// The prepare step shares this vector and reports "passed" on success, so the
/// old `any(passed) && !any(failed)` form returned true for
/// `[prepare: passed, npm test: not-run]` — the shape produced when a run is
/// cancelled during the first real command. A cancelled verification must never
/// read as a passing one.
pub(crate) fn verification_passed(results: &[VerificationResult]) -> bool {
    let mut checked_any = false;
    for result in results
        .iter()
        .filter(|result| result.label != "Prepare dependencies")
    {
        if result.status != "passed" {
            return false;
        }
        checked_any = true;
    }
    checked_any
        && !results
            .iter()
            .any(|result| matches!(result.status.as_str(), "failed" | "unavailable"))
}

pub(crate) fn verification_is_unconfigured(results: &[VerificationResult]) -> bool {
    results.len() == 1 && results[0].label == "Project checks" && results[0].status == "not-run"
}

pub(crate) fn verification_summary(results: &[VerificationResult]) -> String {
    results
        .iter()
        .map(|result| format!("- {}: {}\n  {}", result.label, result.status, result.detail))
        .collect::<Vec<_>>()
        .join("\n")
}
