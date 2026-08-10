use crate::*;

#[cfg(windows)]
fn prepare_owned_provider_command(command: &mut Command) {
    use windows_sys::Win32::System::Threading::CREATE_SUSPENDED;

    // Suspending at creation closes the gap between process creation and Job
    // Object assignment. The provider cannot spawn an unowned descendant before
    // the coordinator has attached it to the kill-on-close job.
    command.creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED);
}

#[cfg(not(windows))]
fn prepare_owned_provider_command(_command: &mut Command) {}

pub(crate) async fn preallocate_cursor_session(executable: &Path) -> Result<String, String> {
    let mut command = Command::new(executable);
    hide_tokio_command_window(&mut command);
    prepare_owned_provider_command(&mut command);
    command
        .arg("create-chat")
        .kill_on_drop(true)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("Could not create a Cursor chat: {error}"))?;
    #[cfg(windows)]
    let _provider_job = match ProviderJob::attach_and_resume(&child) {
        Ok(job) => job,
        Err(error) => {
            kill_tree(&mut child).await;
            let _ = child.wait().await;
            return Err(error);
        }
    };
    let output = timeout(Duration::from_secs(15), child.wait_with_output())
        .await
        .map_err(|_| "Cursor chat creation timed out after 15 seconds.".to_owned())?
        .map_err(|error| format!("Could not create a Cursor chat: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Cursor could not create a chat: {}",
            truncate_utf8(&String::from_utf8_lossy(&output.stderr), 500)
        ));
    }
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "Cursor did not return a chat ID from create-chat.".to_owned())
}

pub(crate) async fn wait_for_idle(
    mut activity: watch::Receiver<u64>,
    timeout_seconds: u64,
) -> bool {
    loop {
        tokio::select! {
            changed = activity.changed() => {
                if changed.is_err() {
                    return false;
                }
            }
            _ = sleep(Duration::from_secs(timeout_seconds)) => return true,
        }
    }
}

pub(crate) fn provider_fatal_stderr(kind: &str, line: &str) -> bool {
    if kind != "cursor" {
        return false;
    }
    let normalized = line.trim().to_ascii_lowercase();
    normalized.starts_with("cannot use this model:")
        || normalized.starts_with("failed to load models:")
        || [
            "authentication required",
            "invalid api key",
            "login required",
            "not logged in",
            "unauthorized",
        ]
        .iter()
        .any(|signal| normalized.contains(signal))
}

pub(crate) fn ensure_cursor_repository_has_head(repository: &Path) -> Result<(), String> {
    match repository_has_head(repository)? {
        true => Ok(()),
        false => Err(
            "Cursor Agent requires the attached Git repository to have an initial commit. Create an initial commit in the repository, then retry."
                .to_owned(),
        ),
    }
}

/// Owns the complete provider process tree on Windows. Provider CLIs can spawn a
/// worker and let their direct wrapper exit, so following only the original PID
/// is not enough to guarantee cleanup. The job's close limit terminates every
/// process still assigned to it when the provider invocation ends.
#[cfg(windows)]
struct ProviderJob(usize);

#[cfg(windows)]
impl ProviderJob {
    fn attach_and_resume(child: &tokio::process::Child) -> Result<Self, String> {
        let job = Self::attach(child)?;
        Self::resume_primary_thread(child)?;
        Ok(job)
    }

    fn attach(child: &tokio::process::Child) -> Result<Self, String> {
        use windows_sys::Win32::{
            Foundation::HANDLE,
            System::JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
                SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            },
        };

        let process = child.raw_handle().ok_or_else(|| {
            "Provider exited before process ownership could be established.".to_owned()
        })? as HANDLE;
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(format!(
                "Could not create the provider process job: {}",
                std::io::Error::last_os_error()
            ));
        }
        let job = Self(handle as usize);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configured = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                std::ptr::addr_of!(limits).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured == 0 {
            return Err(format!(
                "Could not configure provider process cleanup: {}",
                std::io::Error::last_os_error()
            ));
        }
        if unsafe { AssignProcessToJobObject(handle, process) } == 0 {
            return Err(format!(
                "Could not take ownership of the provider process: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(job)
    }

    fn resume_primary_thread(child: &tokio::process::Child) -> Result<(), String> {
        use windows_sys::Win32::{
            Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
            System::{
                Diagnostics::ToolHelp::{
                    CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD,
                    THREADENTRY32,
                },
                Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME},
            },
        };

        let process_id = child.id().ok_or_else(|| {
            "Provider exited before its suspended thread could be resumed.".to_owned()
        })?;
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(format!(
                "Could not inspect the suspended provider thread: {}",
                std::io::Error::last_os_error()
            ));
        }

        let result = (|| {
            let mut entry = THREADENTRY32 {
                dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
                ..Default::default()
            };
            if unsafe { Thread32First(snapshot, std::ptr::addr_of_mut!(entry)) } == 0 {
                return Err(format!(
                    "Could not enumerate the suspended provider thread: {}",
                    std::io::Error::last_os_error()
                ));
            }
            loop {
                if entry.th32OwnerProcessID == process_id {
                    let thread =
                        unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
                    if thread.is_null() {
                        return Err(format!(
                            "Could not open the suspended provider thread: {}",
                            std::io::Error::last_os_error()
                        ));
                    }
                    let resumed = unsafe { ResumeThread(thread) };
                    unsafe {
                        CloseHandle(thread);
                    }
                    if resumed == u32::MAX {
                        return Err(format!(
                            "Could not resume the owned provider process: {}",
                            std::io::Error::last_os_error()
                        ));
                    }
                    return Ok(());
                }
                if unsafe { Thread32Next(snapshot, std::ptr::addr_of_mut!(entry)) } == 0 {
                    return Err(
                        "Could not find the primary thread for the suspended provider process."
                            .to_owned(),
                    );
                }
            }
        })();
        unsafe {
            CloseHandle(snapshot);
        }
        result
    }
}

#[cfg(windows)]
impl Drop for ProviderJob {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};

        unsafe {
            CloseHandle(self.0 as HANDLE);
        }
    }
}

/// On Windows `Child::kill` is a `TerminateProcess` on the direct child only.
/// Providers resolve to `.cmd` shims and verification runs through `cmd /C`, so
/// killing the child leaves the real worker (node.exe, the agent runtime) alive
/// and still holding handles inside the managed worktree — which then fails to
/// remove, and surfaces only as a cleanup warning.
#[cfg(windows)]
pub(crate) async fn kill_pid_tree(pid: Option<u32>) {
    if let Some(pid) = pid {
        let pid_text = pid.to_string();
        let mut command = Command::new("taskkill");
        hide_tokio_command_window(&mut command);
        let _ = command
            .args(["/T", "/F", "/PID", pid_text.as_str()])
            .status()
            .await;
    }
}

#[cfg(not(windows))]
pub(crate) async fn kill_pid_tree(_pid: Option<u32>) {}

pub(crate) async fn kill_tree(child: &mut tokio::process::Child) {
    kill_pid_tree(child.id()).await;
    let _ = child.kill().await;
}

async fn stop_for_idle(
    child: &mut tokio::process::Child,
) -> Result<(std::process::ExitStatus, bool), String> {
    if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
        return Ok((status, false));
    }
    kill_tree(child).await;
    child
        .wait()
        .await
        .map(|status| (status, true))
        .map_err(|error| error.to_string())
}

fn cancellation_requested(
    cancellation: &watch::Receiver<bool>,
    changed: Result<(), watch::error::RecvError>,
) -> bool {
    changed.is_ok() && *cancellation.borrow()
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn invoke_provider(
    app: &AppHandle,
    run_id: &str,
    participant: &Participant,
    phase: Phase,
    mode: ProviderMode,
    prompt: &str,
    repository: &Path,
    session_id: Option<&str>,
    requested_model: Option<&str>,
    requested_effort: Option<&str>,
    final_output_path: &Path,
    mut cancellation: watch::Receiver<bool>,
) -> Result<ProviderRun, String> {
    let kind = participant.kind.as_str();
    let executable = find_provider_executable(kind)
        .ok_or_else(|| format!("{} CLI is not installed.", provider_names(kind).0))?;
    let structured_chat = phase != Phase::Chat || participant.capabilities.structured_output;
    if phase != Phase::Chat
        && matches!(
            participant.capabilities.autonomy_mode.as_str(),
            "manual" | "unavailable"
        )
    {
        return Err(format!(
            "{} cannot prove a safe unattended mode: {}",
            participant.name, participant.capabilities.autonomy_note
        ));
    }
    if kind == "cursor" {
        ensure_cursor_repository_has_head(repository)?;
    }
    let artifact_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())?
        .join("runs")
        .join(run_id);
    std::fs::create_dir_all(&artifact_dir).map_err(|error| error.to_string())?;
    let stdout_log_path = artifact_dir.join(format!("{}.stdout.log", phase.as_str()));
    let stderr_log_path = artifact_dir.join(format!("{}.stderr.log", phase.as_str()));
    let prompt_path = artifact_dir.join(format!("{}.prompt.txt", phase.as_str()));
    std::fs::write(&prompt_path, prompt).map_err(|error| error.to_string())?;
    // Artifact paths are keyed on run_id, and a recovery attempt reuses the run_id.
    // If this phase exits without rewriting its `-o` file, read_to_string below would
    // silently return the *previous* attempt's handoff — including a stale
    // "status":"completed" or a stale review approval. Never inherit one.
    let _ = std::fs::remove_file(final_output_path);
    let argv_prompt = format!(
        "Read the file at {} in full. It contains your complete assignment. Follow it exactly.",
        prompt_path.display()
    );
    let preallocated_cursor_session = if kind == "cursor" && session_id.is_none() {
        Some(preallocate_cursor_session(&executable).await?)
    } else {
        None
    };
    let effective_session = preallocated_cursor_session
        .clone()
        .or_else(|| session_id.map(str::to_owned));
    let effective_session_id = effective_session.as_deref();
    let handoff_contract = (phase != Phase::Chat).then(|| handoff_contract(phase));
    let prepared = providers::build_command(
        kind,
        &TurnRequest {
            mode,
            phase: phase.as_str(),
            prompt: if matches!(kind, "cursor" | "antigravity") && mode != ProviderMode::Probe {
                &argv_prompt
            } else {
                prompt
            },
            repository,
            session_id: effective_session_id,
            model: requested_model,
            effort: requested_effort,
            final_output_path,
            structured_output: structured_chat,
            handoff_contract: handoff_contract.as_deref(),
        },
    )?;
    let assigned_session_id = prepared
        .assigned_session_id
        .clone()
        .or(preallocated_cursor_session.clone());
    let mut command = Command::new(executable);
    command.args(&prepared.args);
    let stdin_prompt = prepared.stdin;

    apply_provider_environment(&mut command, repository);
    command
        .current_dir(repository)
        .kill_on_drop(true)
        .stdin(if stdin_prompt.is_some() {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    prepare_owned_provider_command(&mut command);

    let provider_started = Instant::now();
    let mut child = command
        .spawn()
        .map_err(|error| format!("Failed to start {}: {error}", provider_names(kind).0))?;
    #[cfg(windows)]
    let provider_job = match ProviderJob::attach_and_resume(&child) {
        Ok(job) => job,
        Err(error) => {
            kill_tree(&mut child).await;
            let _ = child.wait().await;
            return Err(error);
        }
    };
    let process_start_ms = provider_started.elapsed().as_millis() as u64;

    if let Some(content) = stdin_prompt {
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(content.as_bytes())
                .await
                .map_err(|error| format!("Failed to send the context packet: {error}"))?;
            let _ = stdin.shutdown().await;
        }
    }

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Provider stdout was unavailable.".to_owned())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Provider stderr was unavailable.".to_owned())?;

    let stdout_app = app.clone();
    let stdout_run = run_id.to_owned();
    let stdout_phase = phase.as_str().to_owned();
    let stdout_agent = kind.to_owned();
    let stderr_agent = kind.to_owned();
    let (activity_sender, activity_receiver) = watch::channel(0_u64);
    let (completion_sender, mut completion_receiver) = watch::channel(false);
    let (fatal_sender, mut fatal_receiver) = watch::channel(false);
    let stdout_activity = activity_sender.clone();
    let completion_phase = phase;
    let stdout_started = provider_started;
    let stdout_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        let mut session_id = None;
        let mut result_text = String::new();
        let mut chat_stream = ChatStreamBuffer::default();
        let mut raw = String::new();
        let mut actual_model = None;
        let mut usage = ProviderUsage::default();
        let mut activity_emitted = false;
        let mut first_output_ms = None;
        let mut stream_flush = tokio::time::interval_at(
            tokio::time::Instant::now() + Duration::from_millis(50),
            Duration::from_millis(50),
        );
        loop {
            let line = tokio::select! {
                line = lines.next_line() => match line {
                    Ok(line) => line,
                    Err(_) => break,
                },
                _ = stream_flush.tick(), if completion_phase == Phase::Chat => {
                    flush_chat_stream(
                        &stdout_app,
                        &stdout_run,
                        &stdout_phase,
                        &stdout_agent,
                        &mut chat_stream,
                    );
                    continue;
                }
            };
            let Some(line) = line else {
                break;
            };
            first_output_ms.get_or_insert_with(|| stdout_started.elapsed().as_millis() as u64);
            stdout_activity.send_modify(|value| *value = value.saturating_add(1));
            append_capped(&mut raw, &line);
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                session_id = session_id.or_else(|| parse_session_id(&value));
                actual_model = actual_model.or_else(|| reported_model(&value));
                merge_usage(&mut usage, &value);
                let thinking_fragment = (completion_phase == Phase::Chat)
                    .then(|| provider_thinking_fragment(&stdout_agent, &value))
                    .flatten();
                if let Some(fragment) = thinking_fragment {
                    chat_stream.push_thinking(fragment);
                    activity_emitted = true;
                } else if let Some((title, detail)) = provider_activity(&stdout_agent, &value) {
                    emit_event(
                        &stdout_app,
                        &stdout_run,
                        "stream",
                        &stdout_phase,
                        "running",
                        Some(&stdout_agent),
                        &title,
                        &detail,
                        None,
                    );
                    activity_emitted = true;
                }
                if completion_phase == Phase::Chat {
                    if let Some(fragment) = provider_chat_fragment(&stdout_agent, &value) {
                        chat_stream.push_fragment(&fragment);
                    } else if let Some(text) = provider_chat_text(&stdout_agent, &value) {
                        chat_stream.replace_snapshot(text);
                    } else if let Some(text) = parse_result_text(&value) {
                        chat_stream.replace_snapshot(text);
                    }
                } else if let Some(text) = parse_result_text(&value) {
                    result_text = text;
                }
            } else if completion_phase == Phase::Chat && !structured_chat {
                chat_stream.push_fragment(&format!("{line}\n"));
            }
            if raw.contains(HANDOFF_END) && extract_phase_handoff(&raw, completion_phase).is_ok() {
                completion_sender.send_replace(true);
            }
            if completion_phase == Phase::Chat {
                if chat_stream.result_text.trim().is_empty() && !activity_emitted {
                    emit_event(
                        &stdout_app,
                        &stdout_run,
                        "stream",
                        &stdout_phase,
                        "running",
                        Some(&stdout_agent),
                        "Chat activity",
                        &format!(
                            "{} is working in the attached repository.",
                            provider_names(&stdout_agent).0
                        ),
                        None,
                    );
                    activity_emitted = true;
                }
            } else if !line.trim_start().starts_with('{') {
                emit_event(
                    &stdout_app,
                    &stdout_run,
                    "stream",
                    &stdout_phase,
                    "running",
                    Some(&stdout_agent),
                    "Provider activity",
                    &truncate_utf8(&line, 2 * 1024),
                    None,
                );
            }
        }
        if completion_phase == Phase::Chat {
            flush_chat_stream(
                &stdout_app,
                &stdout_run,
                &stdout_phase,
                &stdout_agent,
                &mut chat_stream,
            );
        }
        (
            session_id,
            if completion_phase == Phase::Chat {
                chat_stream.result_text
            } else {
                result_text
            },
            raw,
            actual_model,
            usage,
            first_output_ms,
        )
    });

    let stderr_activity = activity_sender;
    let stderr_fatal = fatal_sender;
    let stderr_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        let mut raw = String::new();
        while let Ok(Some(line)) = lines.next_line().await {
            stderr_activity.send_modify(|value| *value = value.saturating_add(1));
            append_capped(&mut raw, &line);
            if provider_fatal_stderr(&stderr_agent, &line) {
                stderr_fatal.send_replace(true);
            }
        }
        raw
    });

    let mut stopped = false;
    let mut timed_out = false;
    let mut idle_timed_out = false;
    let mut completed_handoff = false;
    let status = tokio::select! {
        result = child.wait() => result.map_err(|error| error.to_string())?,
        changed = cancellation.changed() => {
            if cancellation_requested(&cancellation, changed) {
                stopped = true;
                kill_tree(&mut child).await;
            }
            child.wait().await.map_err(|error| error.to_string())?
        },
        _ = sleep(Duration::from_secs(PROCESS_TIMEOUT_SECONDS)) => {
            timed_out = true;
            kill_tree(&mut child).await;
            child.wait().await.map_err(|error| error.to_string())?
        },
        reached_idle_timeout = wait_for_idle(activity_receiver, idle_timeout_seconds(phase)) => {
            if reached_idle_timeout {
                let (result, was_idle_timeout) = stop_for_idle(&mut child).await?;
                idle_timed_out = was_idle_timeout;
                result
            } else {
                child.wait().await.map_err(|error| error.to_string())?
            }
        },
        changed = completion_receiver.changed() => {
            if changed.is_ok() && *completion_receiver.borrow() {
                completed_handoff = true;
                kill_tree(&mut child).await;
            }
            child.wait().await.map_err(|error| error.to_string())?
        }
        changed = fatal_receiver.changed() => {
            if changed.is_ok() && *fatal_receiver.borrow() {
                kill_tree(&mut child).await;
            }
            child.wait().await.map_err(|error| error.to_string())?
        }
    };

    // The direct wrapper has ended. Closing the job now removes any detached
    // workers before log readers are joined or the managed worktree is cleaned.
    #[cfg(windows)]
    drop(provider_job);

    let (parsed_session, parsed_result, raw_stdout, actual_model, usage, first_output_ms) =
        stdout_task.await.map_err(|error| error.to_string())?;
    let raw_stderr = stderr_task.await.map_err(|error| error.to_string())?;
    std::fs::write(&stdout_log_path, &raw_stdout).map_err(|error| error.to_string())?;
    std::fs::write(&stderr_log_path, &raw_stderr).map_err(|error| error.to_string())?;
    let file_result = std::fs::read_to_string(final_output_path).unwrap_or_default();
    let raw_result = if !file_result.trim().is_empty() {
        file_result.trim().to_owned()
    } else if !parsed_result.trim().is_empty() {
        parsed_result.trim().to_owned()
    } else if phase != Phase::Chat || !structured_chat {
        raw_stdout.trim().to_owned()
    } else {
        String::new()
    };
    let (handoff, schema_error) = if phase == Phase::Chat {
        (None, None)
    } else {
        match extract_phase_handoff(&raw_result, phase) {
            Ok(handoff) => (Some(handoff), None),
            Err(error) => (None, Some(error)),
        }
    };
    let summary = handoff
        .as_ref()
        .map(|value| value.summary.clone())
        .unwrap_or_else(|| {
            if phase == Phase::Chat {
                raw_result.clone()
            } else {
                truncate_utf8(&raw_result, SOURCE_BUDGET_BYTES)
            }
        });
    let logical_success = if phase == Phase::Chat {
        chat_response_is_complete(&raw_result)
    } else {
        handoff.as_ref().is_some_and(|value| match phase {
            Phase::Build | Phase::Revise => value.status == "completed",
            Phase::Review | Phase::FinalReview => {
                matches!(value.status.as_str(), "approved" | "changes_required")
            }
            Phase::Chat => false,
        })
    };
    let diagnostic = [
        raw_stderr.trim(),
        schema_error.as_deref().unwrap_or_default(),
    ]
    .into_iter()
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>()
    .join("\n");

    let session_resumed = participant.capabilities.exact_resume
        && effective_session_id.is_some()
        && parsed_session.as_deref() == effective_session_id;
    Ok(ProviderRun {
        summary,
        session_id: parsed_session.or(assigned_session_id),
        success: (status.success()
            || completed_handoff
            || (phase == Phase::Chat && idle_timed_out && logical_success))
            && !stopped
            && !timed_out
            && (!idle_timed_out || (phase == Phase::Chat && logical_success))
            && logical_success,
        stopped,
        timed_out,
        idle_timed_out,
        stderr: diagnostic,
        handoff,
        actual_model,
        usage,
        stdout_log_path: stdout_log_path.to_string_lossy().into_owned(),
        stderr_log_path: stderr_log_path.to_string_lossy().into_owned(),
        process_start_ms,
        first_output_ms,
        session_resumed,
    })
}

pub(crate) fn provider_log_note(run: &ProviderRun) -> String {
    format!(
        "Durable logs:\nstdout: {}\nstderr: {}",
        run.stdout_log_path, run.stderr_log_path
    )
}

pub(crate) fn provider_failure_reason(provider: &str, activity: &str, run: &ProviderRun) -> String {
    let stderr = run.stderr.trim();
    if !stderr.is_empty() {
        return format!(
            "{provider} could not complete {activity}: {}. {}",
            truncate_utf8(stderr, 500),
            provider_log_note(run)
        );
    }
    format!(
        "{provider} could not complete {activity}. {}",
        provider_log_note(run)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dropped_cancellation_sender_does_not_mark_run_stopped() {
        let (sender, mut cancellation) = watch::channel(false);
        drop(sender);

        let changed = cancellation.changed().await;

        assert!(!cancellation_requested(&cancellation, changed));
    }

    #[tokio::test]
    async fn closed_activity_channel_is_not_an_idle_timeout() {
        let (sender, receiver) = watch::channel(0_u64);
        drop(sender);

        let reached_idle_timeout = wait_for_idle(receiver, 60).await;

        assert!(!reached_idle_timeout);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn fast_nonzero_exit_with_stderr_is_not_classified_as_idle_timeout() {
        let mut command = Command::new("cmd");
        command
            .args(["/C", "echo sandbox is unavailable 1>&2 & exit /b 1"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped());
        let mut child = command.spawn().expect("spawn fast failing child");
        let stderr = child.stderr.take().expect("capture stderr");
        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            let mut output = String::new();
            while let Some(line) = lines.next_line().await.expect("read stderr") {
                append_capped(&mut output, &line);
            }
            output
        });

        let status = timeout(Duration::from_secs(2), child.wait())
            .await
            .expect("fast child exit")
            .expect("wait for fast child");
        assert!(!status.success());
        let (status, idle_timed_out) = stop_for_idle(&mut child)
            .await
            .expect("collect already-exited child");
        let stderr = stderr_task.await.expect("join stderr task");

        assert!(!status.success());
        assert!(!idle_timed_out);
        let run = ProviderRun {
            summary: String::new(),
            session_id: None,
            success: false,
            stopped: false,
            timed_out: false,
            idle_timed_out,
            stderr,
            handoff: None,
            actual_model: None,
            usage: ProviderUsage::default(),
            stdout_log_path: "stdout.log".to_owned(),
            stderr_log_path: "stderr.log".to_owned(),
            process_start_ms: 0,
            first_output_ms: None,
            session_resumed: false,
        };
        let reason = provider_failure_reason("Cursor Agent", "the connection test", &run);
        assert!(reason.contains("sandbox is unavailable"));
        assert!(!reason.contains("No provider output"));
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn suspended_provider_job_owns_a_descendant_after_the_wrapper_exits() {
        let directory =
            std::env::temp_dir().join(format!("staff-room-provider-job-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).expect("create test directory");
        let child_script = directory.join("child.ps1");
        let spawned_marker = directory.join("spawned.txt");
        let started_marker = directory.join("started.txt");
        let escaped = |path: &Path| path.to_string_lossy().replace('\'', "''");
        std::fs::write(
            &child_script,
            format!(
                "Set-Content -LiteralPath '{}' -Value started\nStart-Sleep -Seconds 30\n",
                escaped(&started_marker)
            ),
        )
        .expect("write descendant script");
        let parent_script = format!(
            "$ErrorActionPreference = 'Stop'\n$child = Start-Process -FilePath 'powershell.exe' -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', '{}') -PassThru\nSet-Content -LiteralPath '{}' -Value $child.Id",
            escaped(&child_script),
            escaped(&spawned_marker)
        );
        let mut command = Command::new("powershell.exe");
        command
            .args(["-NoProfile", "-Command", &parent_script])
            .kill_on_drop(true)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit());
        prepare_owned_provider_command(&mut command);
        let mut child = command.spawn().expect("spawn provider parent");
        let job = ProviderJob::attach_and_resume(&child).expect("own and resume provider parent");

        let descendant_pid = timeout(Duration::from_secs(20), async {
            loop {
                if let Ok(contents) = std::fs::read_to_string(&spawned_marker) {
                    if let Ok(pid) = contents.trim().parse::<u32>() {
                        if started_marker.exists() {
                            break pid;
                        }
                    }
                }
                sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .expect("a valid descendant PID was recorded");
        assert_ne!(descendant_pid, 0, "descendant PID was nonzero");

        timeout(Duration::from_secs(20), child.wait())
            .await
            .expect("provider wrapper exited")
            .expect("wait for provider wrapper");
        use windows_sys::Win32::{
            Foundation::{CloseHandle, WAIT_OBJECT_0},
            System::Threading::{
                OpenProcess, TerminateProcess, WaitForSingleObject, PROCESS_TERMINATE,
            },
        };
        const PROCESS_SYNCHRONIZE: u32 = 0x0010_0000;
        let descendant =
            unsafe { OpenProcess(PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, 0, descendant_pid) };
        assert!(
            !descendant.is_null(),
            "started descendant remained available before the job closed"
        );
        drop(job);
        let wait_result = unsafe { WaitForSingleObject(descendant, 5_000) };
        if wait_result != WAIT_OBJECT_0 {
            unsafe {
                TerminateProcess(descendant, 1);
                WaitForSingleObject(descendant, 5_000);
            }
        }
        unsafe {
            CloseHandle(descendant);
        }
        assert_eq!(
            wait_result, WAIT_OBJECT_0,
            "detached descendant survived the provider job"
        );
        std::fs::remove_dir_all(&directory).expect("remove test directory");
    }
}
