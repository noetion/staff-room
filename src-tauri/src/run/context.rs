use crate::*;

pub(crate) fn assemble_packet(budget: usize, sections: &[(&str, String)]) -> (String, usize) {
    let mut packet = String::new();
    for (title, content) in sections {
        if content.trim().is_empty() {
            continue;
        }
        let remaining = budget.saturating_sub(packet.len());
        if remaining < 128 {
            packet.push_str("\n\n[context budget reached]");
            break;
        }
        let section = format!(
            "\n\n# {title}\n{}",
            truncate_utf8(content, SOURCE_BUDGET_BYTES)
        );
        packet.push_str(&truncate_utf8(&section, remaining));
    }
    let bytes = packet.len();
    (packet.trim().to_owned(), bytes)
}

pub(crate) fn handoff_contract(phase: Phase) -> String {
    let statuses = if matches!(phase, Phase::Review | Phase::FinalReview) {
        "`approved` or `changes_required`"
    } else {
        "`completed`, `blocked`, or `failed`"
    };
    format!(
        "Your final response must end with exactly this machine-readable record. Do not place Markdown fences around it.\n\
         {HANDOFF_START}\n\
         {{\"schemaVersion\":1,\"status\":\"STATUS\",\"summary\":\"concise factual summary\",\"changedFiles\":[\"path\"],\"checks\":[\"command: result\"],\"findings\":[\"material finding\"],\"nextAction\":\"one explicit next action or none\"}}\n\
         {HANDOFF_END}\n\
         Allowed status values for this phase: {statuses}. All fields are required. Use empty arrays when needed."
    )
}

pub(crate) fn extract_handoff(value: &str) -> Result<AgentHandoff, String> {
    let start = value
        .rfind(HANDOFF_START)
        .ok_or_else(|| format!("Missing {HANDOFF_START} marker."))?;
    let body_start = start + HANDOFF_START.len();
    let remaining = &value[body_start..];
    let end = remaining
        .find(HANDOFF_END)
        .ok_or_else(|| format!("Missing {HANDOFF_END} marker."))?;
    let json = remaining[..end].trim();
    let handoff = serde_json::from_str::<AgentHandoff>(json)
        .map_err(|error| format!("Invalid handoff JSON: {error}"))?;
    if handoff.schema_version != 1 {
        return Err(format!(
            "Unsupported handoff schema version {}.",
            handoff.schema_version
        ));
    }
    if handoff.summary.trim().is_empty() || handoff.next_action.trim().is_empty() {
        return Err("Handoff summary and nextAction must be non-empty.".to_owned());
    }
    if !matches!(
        handoff.status.as_str(),
        "completed" | "blocked" | "failed" | "approved" | "changes_required"
    ) {
        return Err(format!("Unsupported handoff status `{}`.", handoff.status));
    }
    Ok(handoff)
}

pub(crate) fn handoff_status_allowed(handoff: &AgentHandoff, phase: Phase) -> bool {
    match phase {
        Phase::Chat => false,
        Phase::Build | Phase::Revise => {
            matches!(handoff.status.as_str(), "completed" | "blocked" | "failed")
        }
        Phase::Review | Phase::FinalReview => {
            matches!(handoff.status.as_str(), "approved" | "changes_required")
        }
    }
}

pub(crate) fn handoff_attention_reason(handoff: &AgentHandoff) -> String {
    let findings = if handoff.findings.is_empty() {
        String::new()
    } else {
        format!("\n{}", handoff.findings.join("\n"))
    };
    format!(
        "{}{findings}\nNext action: {}",
        handoff.summary, handoff.next_action
    )
}

pub(crate) fn extract_phase_handoff(value: &str, phase: Phase) -> Result<AgentHandoff, String> {
    if phase == Phase::Chat {
        return Err("Chat responses do not require a Staff Room handoff.".to_owned());
    }
    let handoff = extract_handoff(value)?;
    if handoff_status_allowed(&handoff, phase) {
        Ok(handoff)
    } else {
        Err(format!(
            "Handoff status `{}` is invalid for {}.",
            handoff.status,
            phase.as_str()
        ))
    }
}

pub(crate) fn extract_ship_intent(value: &str) -> Result<Option<ShipIntent>, String> {
    let Some(start) = value.find(SHIP_INTENT_START) else {
        return Ok(None);
    };
    let json_start = start + SHIP_INTENT_START.len();
    let end = value[json_start..]
        .find(SHIP_INTENT_END)
        .map(|offset| json_start + offset)
        .ok_or_else(|| "Autonomous Ship intent is missing its closing marker.".to_owned())?;
    let intent = serde_json::from_str::<ShipIntent>(value[json_start..end].trim())
        .map_err(|error| format!("Invalid autonomous Ship intent: {error}"))?;
    if intent.schema_version != 1 {
        return Err(format!(
            "Unsupported autonomous Ship intent schema version {}.",
            intent.schema_version
        ));
    }
    if intent.objective.trim().is_empty() || intent.reason.trim().is_empty() {
        return Err("Autonomous Ship intent requires an objective and reason.".to_owned());
    }
    Ok(Some(intent))
}

pub(crate) fn visible_chat_response(value: &str) -> String {
    let Some(start) = value.find(SHIP_INTENT_START) else {
        return value.trim().to_owned();
    };
    let json_start = start + SHIP_INTENT_START.len();
    let Some(end_offset) = value[json_start..].find(SHIP_INTENT_END) else {
        let before = value[..start].trim();
        return if before.is_empty() {
            "The provider returned an invalid autonomous Ship intent, so no repository work was started."
                .to_owned()
        } else {
            before.to_owned()
        };
    };
    let end = json_start + end_offset + SHIP_INTENT_END.len();
    let visible = format!("{}\n{}", value[..start].trim(), value[end..].trim())
        .trim()
        .to_owned();
    if visible.is_empty() {
        "This request is ready for the autonomous Ship workflow.".to_owned()
    } else {
        visible
    }
}

pub(crate) fn authentication_attention(summary: &str, stderr: &str) -> Option<String> {
    let summary = summary.to_ascii_lowercase();
    let stderr = stderr.to_ascii_lowercase();
    let onboarding = [
        "you are currently not signed in",
        "signing in...",
        "terms of service & data use",
        "welcome to antigravity cli!",
    ];
    let provider_error = ["authentication required", "not logged in", "oauth"];
    if onboarding.iter().any(|signal| summary.contains(signal))
        || onboarding.iter().any(|signal| stderr.contains(signal))
        || provider_error.iter().any(|signal| stderr.contains(signal))
    {
        Some(
            "The provider requires sign-in or onboarding before The Staff Room can use it."
                .to_owned(),
        )
    } else {
        None
    }
}

pub(crate) fn connection_test_ready(result: &ProviderRun) -> bool {
    result.success
        && authentication_attention(&result.summary, &result.stderr).is_none()
        && result.summary.trim() == "READY"
}

pub(crate) fn cache_declared_capabilities(
    database: &Database,
    participants: &[Participant],
) -> Result<(), String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    for participant in participants
        .iter()
        .filter(|participant| participant.installed)
    {
        let Some(version) = participant.version.as_deref() else {
            continue;
        };
        connection
            .execute(
                "INSERT OR IGNORE INTO provider_capabilities (provider, version, capabilities_json) VALUES (?1, ?2, ?3)",
                params![participant.kind, version, serde_json::to_string(&participant.capabilities).map_err(|error| error.to_string())?],
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(crate) fn review_handoff_decision(run: &ProviderRun) -> Option<bool> {
    match run.handoff.as_ref()?.status.as_str() {
        "approved" => Some(true),
        "changes_required" => Some(false),
        _ => None,
    }
}
