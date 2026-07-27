use crate::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_event(
    app: &AppHandle,
    run_id: &str,
    event_type: &str,
    phase: &str,
    state: &str,
    agent: Option<&str>,
    title: &str,
    detail: &str,
    context_bytes: Option<usize>,
) {
    let _ = app.emit(
        "run-event",
        RunEvent {
            run_id: run_id.to_owned(),
            event_type: event_type.to_owned(),
            phase: phase.to_owned(),
            state: state.to_owned(),
            agent: agent.map(str::to_owned),
            title: title.to_owned(),
            detail: detail.to_owned(),
            text_delta: None,
            context_bytes,
        },
    );
}

pub(crate) fn emit_text_delta(
    app: &AppHandle,
    run_id: &str,
    phase: &str,
    agent: &str,
    text: String,
) {
    let _ = app.emit(
        "run-event",
        RunEvent {
            run_id: run_id.to_owned(),
            event_type: "text-delta".to_owned(),
            phase: phase.to_owned(),
            state: "running".to_owned(),
            agent: Some(agent.to_owned()),
            title: "Chat response".to_owned(),
            detail: String::new(),
            text_delta: Some(text),
            context_bytes: None,
        },
    );
}

pub(crate) fn flush_chat_stream(
    app: &AppHandle,
    run_id: &str,
    phase: &str,
    agent: &str,
    stream: &mut ChatStreamBuffer,
) {
    if let Some(text) = stream.take_text_delta() {
        emit_text_delta(app, run_id, phase, agent, text);
    }
    if stream.thinking_updated {
        emit_event(
            app,
            run_id,
            "stream",
            phase,
            "running",
            Some(agent),
            "Thinking",
            &truncate_utf8(&stream.thinking, 2 * 1024),
            None,
        );
        stream.thinking_updated = false;
    }
}
