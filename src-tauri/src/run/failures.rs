use crate::*;

pub(crate) fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn final_failure(
    app: &AppHandle,
    database: &Database,
    project_id: &str,
    run_id: &str,
    state: &str,
    reason: &str,
    worktree: Option<&Path>,
    review_count: u32,
    revision_count: u32,
    context_bytes: usize,
) -> Result<(), String> {
    let recovery = worktree.map(|path| path.to_string_lossy().into_owned());
    let detail = if let Some(path) = recovery.as_deref() {
        format!("{reason}\nRecoverable worktree: {path}")
    } else {
        reason.to_owned()
    };
    persist_message(
        database,
        project_id,
        run_id,
        "system",
        if state == "stopped" {
            "status"
        } else {
            "error"
        },
        if state == "stopped" {
            "The run was stopped."
        } else {
            "The Staff Room needs attention."
        },
        &[],
        &[],
        Some(&detail),
    )?;
    update_run(
        database,
        run_id,
        state,
        None,
        review_count,
        revision_count,
        None,
        context_bytes,
        Some(&detail),
        true,
    )?;
    emit_event(
        app,
        run_id,
        "attention",
        "complete",
        state,
        None,
        if state == "stopped" {
            "Run stopped"
        } else {
            "Attention required"
        },
        &detail,
        Some(context_bytes),
    );
    notify(
        app,
        if state == "stopped" {
            "The Staff Room stopped"
        } else {
            "The Staff Room needs attention"
        },
        &truncate_utf8(&detail, 240),
    );
    Ok(())
}
