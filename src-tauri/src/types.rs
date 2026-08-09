use crate::*;

pub(crate) const BUILD_CONTEXT_BUDGET_BYTES: usize = 48 * 1024;
pub(crate) const ROOM_MESSAGE_PAGE_SIZE: usize = 100;
pub(crate) const SOURCE_BUDGET_BYTES: usize = 16 * 1024;
pub(crate) const CHAT_TIMELINE_BUDGET_BYTES: usize = 32 * 1024;
pub(crate) const PROCESS_OUTPUT_LIMIT: usize = 2 * 1024 * 1024;
pub(crate) const PROCESS_TIMEOUT_SECONDS: u64 = 20 * 60;
pub(crate) const PROCESS_IDLE_TIMEOUT_SECONDS: u64 = 5 * 60;
pub(crate) const MAX_RECOVERY_ATTEMPTS: u32 = 2;
pub(crate) const MAX_SELECTED_SKILLS: usize = 3;
pub(crate) const CHAT_HANDOFF_BUDGET_BYTES: usize = 4 * 1024;
pub(crate) const HANDOFF_START: &str = "STAFF_ROOM_RESULT_START";
pub(crate) const HANDOFF_END: &str = "STAFF_ROOM_RESULT_END";
pub(crate) const SHIP_INTENT_START: &str = "STAFF_ROOM_SHIP_INTENT_START";
pub(crate) const SHIP_INTENT_END: &str = "STAFF_ROOM_SHIP_INTENT_END";
pub(crate) const AUTONOMOUS_SHIP_SKILL_PATH: &str = ".agents/skills/autonomous-ship/SKILL.md";
pub(crate) const AUTONOMOUS_SHIP_SKILL: &str =
    include_str!("../../.agents/skills/autonomous-ship/SKILL.md");
#[cfg(windows)]
pub(crate) const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub(crate) struct Database(pub(crate) Mutex<Connection>);

#[derive(Default)]
pub(crate) struct RuntimeState {
    pub(crate) cancellations: AsyncMutex<HashMap<String, CancellationEntry>>,
    pub(crate) active_ship_runs: AsyncMutex<HashSet<String>>,
    pub(crate) provider_cache: AsyncMutex<HashMap<String, Participant>>,
    pub(crate) quick_edits: AsyncMutex<HashMap<String, QuickEditState>>,
    pub(crate) issued_operations: AsyncMutex<HashMap<String, IssuedOperation>>,
    pub(crate) active_promotions: AsyncMutex<HashSet<String>>,
    pub(crate) voice_capture: AsyncMutex<Option<VoiceCaptureSession>>,
}

#[derive(Clone)]
pub(crate) struct CancellationEntry {
    pub(crate) project_id: String,
    pub(crate) sender: watch::Sender<bool>,
}

#[derive(Clone)]
pub(crate) struct IssuedOperation {
    pub(crate) project_id: String,
    pub(crate) kind: String,
}

#[derive(Clone)]
pub(crate) struct QuickEditState {
    pub(crate) project_id: String,
    pub(crate) repository: PathBuf,
    pub(crate) worktree: PathBuf,
    pub(crate) reviewed_diff: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderCapabilities {
    pub(crate) non_interactive_turn: bool,
    pub(crate) streaming: bool,
    pub(crate) structured_output: bool,
    pub(crate) exact_resume: bool,
    pub(crate) cancellation: bool,
    pub(crate) write_mode: bool,
    pub(crate) approval_bridge: bool,
    pub(crate) usage_reporting: bool,
    pub(crate) repository_scoping: bool,
    pub(crate) warm_session: bool,
    pub(crate) autonomy_mode: String,
    pub(crate) autonomy_note: String,
    pub(crate) capability_proof: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Participant {
    pub(crate) kind: String,
    pub(crate) name: String,
    pub(crate) installed: bool,
    pub(crate) version: Option<String>,
    pub(crate) executable_path: Option<String>,
    pub(crate) models: Vec<String>,
    pub(crate) model_discovery_note: String,
    pub(crate) supports_effort: bool,
    pub(crate) effort_options: Vec<String>,
    pub(crate) state: String,
    pub(crate) connection_status: String,
    pub(crate) connection_detail: String,
    pub(crate) last_verified_at: Option<String>,
    pub(crate) capabilities: ProviderCapabilities,
}

#[derive(Debug, Clone)]
pub(crate) struct ProviderConnection {
    pub(crate) status: String,
    pub(crate) detail: String,
    pub(crate) last_verified_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelDiscoveryResult {
    pub(crate) models: Vec<String>,
    pub(crate) detail: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelDiscoveryRequest {
    pub(crate) participant_kind: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeEnvironment {
    pub(crate) native: bool,
    pub(crate) attached: bool,
    pub(crate) repository_path: String,
    pub(crate) branch: String,
    pub(crate) participants: Vec<Participant>,
    pub(crate) context_budget_bytes: usize,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunEvent {
    pub(crate) run_id: String,
    pub(crate) event_type: String,
    pub(crate) phase: String,
    pub(crate) state: String,
    pub(crate) agent: Option<String>,
    pub(crate) title: String,
    pub(crate) detail: String,
    pub(crate) text_delta: Option<String>,
    pub(crate) context_bytes: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StopRunResult {
    pub(crate) cancelled: bool,
    pub(crate) reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AllocateOperationRequest {
    pub(crate) project_id: String,
    pub(crate) kind: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectRunRequest {
    pub(crate) project_id: String,
    pub(crate) run_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PromotionActionResult {
    pub(crate) promoted: bool,
    pub(crate) cleanup_warning: Option<String>,
}

pub(crate) struct VoiceCaptureSession {
    pub(crate) stop: std::sync::mpsc::Sender<()>,
    pub(crate) result: tokio::sync::oneshot::Receiver<Result<CapturedAudio, String>>,
    pub(crate) started: Instant,
    pub(crate) finished: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

pub(crate) struct CapturedAudio {
    pub(crate) samples: Vec<f32>,
    pub(crate) sample_rate: u32,
    pub(crate) channels: u16,
    pub(crate) duration_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VoiceStatus {
    pub(crate) available: bool,
    pub(crate) recording: bool,
    pub(crate) model_path: Option<String>,
    pub(crate) detail: String,
    pub(crate) max_seconds: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VoiceLevel {
    pub(crate) level: f32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VoiceTranscription {
    pub(crate) text: String,
    pub(crate) duration_ms: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VerificationResult {
    pub(crate) label: String,
    pub(crate) status: String,
    pub(crate) detail: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VerificationCommand {
    pub(crate) label: String,
    pub(crate) command: String,
    pub(crate) enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VerificationConfig {
    pub(crate) enabled: bool,
    pub(crate) commands: Vec<VerificationCommand>,
    pub(crate) prepare: Option<String>,
}

impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            commands: Vec::new(),
            prepare: None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VerificationConfigInput {
    pub(crate) project_id: String,
    pub(crate) enabled: bool,
    pub(crate) commands: Vec<VerificationCommand>,
    pub(crate) prepare: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StartRunResult {
    pub(crate) run_id: String,
    pub(crate) state: String,
    pub(crate) summary: String,
    pub(crate) builder: String,
    pub(crate) reviewer: String,
    pub(crate) degraded_review: bool,
    pub(crate) session_id: Option<String>,
    pub(crate) changed_files: Vec<String>,
    pub(crate) git_status: String,
    pub(crate) verification: Vec<VerificationResult>,
    pub(crate) stopped: bool,
    pub(crate) promoted: bool,
    pub(crate) worktree_path: Option<String>,
    pub(crate) branch: String,
    pub(crate) context_bytes: usize,
    pub(crate) attention_reason: Option<String>,
    pub(crate) artifact_path: Option<String>,
    pub(crate) instruction_files: Vec<String>,
    pub(crate) skill_files: Vec<String>,
    pub(crate) recovery_count: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StartRunRequest {
    pub(crate) run_id: String,
    pub(crate) project_id: String,
    pub(crate) objective: String,
    pub(crate) requested_agent: Option<String>,
}

#[derive(Debug)]
pub(crate) struct IsolationContext {
    pub(crate) worktree: PathBuf,
    pub(crate) branch: String,
    pub(crate) base_branch: String,
    pub(crate) base_head: String,
    pub(crate) snapshot_head: String,
    pub(crate) isolation_kind: String,
    pub(crate) workspace_fingerprint: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PromotionMode {
    FastForward,
    WorkingTree,
}

#[derive(Debug)]
pub(crate) struct PromotionResult {
    pub(crate) mode: PromotionMode,
    pub(crate) cleanup_warning: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatRequest {
    pub(crate) run_id: String,
    pub(crate) project_id: String,
    pub(crate) message: String,
    pub(crate) requested_agent: Option<String>,
    pub(crate) active_run_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatResult {
    pub(crate) run_id: String,
    pub(crate) participant: String,
    pub(crate) summary: String,
    pub(crate) session_id: Option<String>,
    pub(crate) actual_model: Option<String>,
    pub(crate) stopped: bool,
    pub(crate) ship_intent: Option<ShipIntent>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuickEditRequest {
    pub(crate) edit_id: String,
    pub(crate) project_id: String,
    pub(crate) message: String,
    pub(crate) requested_agent: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuickEditResult {
    pub(crate) edit_id: String,
    pub(crate) participant: String,
    pub(crate) summary: String,
    pub(crate) diff: String,
    pub(crate) stopped: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuickEditActionRequest {
    pub(crate) edit_id: String,
    pub(crate) project_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuickEditActionResult {
    pub(crate) completed: bool,
    pub(crate) cleanup_warning: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShipIntent {
    pub(crate) schema_version: u32,
    pub(crate) objective: String,
    pub(crate) reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConnectionTestRequest {
    pub(crate) project_id: String,
    pub(crate) participant_kind: String,
    #[serde(default)]
    pub(crate) model: Option<String>,
    #[serde(default)]
    pub(crate) effort: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Project {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) goal: String,
    pub(crate) repository_path: String,
    pub(crate) branch: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectSettings {
    pub(crate) autonomous_ship_enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectSettingsInput {
    pub(crate) project_id: String,
    pub(crate) autonomous_ship_enabled: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredMessage {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) sender: String,
    pub(crate) body: String,
    pub(crate) created_at: String,
    pub(crate) run_id: Option<String>,
    pub(crate) changed_files: Vec<String>,
    pub(crate) verification: Vec<VerificationResult>,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MessageCursor {
    pub(crate) created_at: String,
    pub(crate) id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredRun {
    pub(crate) id: String,
    pub(crate) objective: String,
    pub(crate) state: String,
    pub(crate) current_owner: Option<String>,
    pub(crate) writer: Option<String>,
    pub(crate) reviewer: Option<String>,
    pub(crate) review_count: u32,
    pub(crate) revision_count: u32,
    pub(crate) started_at: String,
    pub(crate) stop_reason: Option<String>,
    pub(crate) native_session_id: Option<String>,
    pub(crate) worktree_path: Option<String>,
    pub(crate) branch: Option<String>,
    pub(crate) context_bytes: usize,
    pub(crate) degraded_review: bool,
    pub(crate) artifact_path: Option<String>,
    pub(crate) instruction_files: Vec<String>,
    pub(crate) skill_files: Vec<String>,
    pub(crate) recovery_count: u32,
    pub(crate) route: Vec<StoredRouteStep>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredRouteStep {
    pub(crate) agent: Option<String>,
    pub(crate) label: String,
    pub(crate) state: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RoomSnapshot {
    pub(crate) messages: Vec<StoredMessage>,
    pub(crate) has_more: bool,
    pub(crate) next_message_cursor: Option<MessageCursor>,
    pub(crate) latest_run: Option<StoredRun>,
    pub(crate) receipts: Vec<ExecutionReceipt>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderUsage {
    pub(crate) input_tokens: Option<u64>,
    pub(crate) cached_input_tokens: Option<u64>,
    pub(crate) output_tokens: Option<u64>,
    pub(crate) total_cost_usd: Option<f64>,
    pub(crate) num_turns: Option<u64>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExecutionReceipt {
    pub(crate) id: String,
    pub(crate) phase: String,
    pub(crate) participant: String,
    pub(crate) provider_version: Option<String>,
    pub(crate) requested_model: Option<String>,
    pub(crate) requested_effort: Option<String>,
    pub(crate) actual_model: Option<String>,
    pub(crate) session_id: Option<String>,
    pub(crate) context_bytes: usize,
    pub(crate) usage: ProviderUsage,
    pub(crate) usage_note: String,
    pub(crate) created_at: String,
    pub(crate) preflight_ms: Option<u64>,
    pub(crate) process_start_ms: Option<u64>,
    pub(crate) first_output_ms: Option<u64>,
    pub(crate) total_ms: Option<u64>,
    pub(crate) session_resumed: bool,
    pub(crate) packet_bytes_saved: usize,
    pub(crate) stdout_log_path: Option<String>,
    pub(crate) stderr_log_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderProfile {
    pub(crate) participant_kind: String,
    pub(crate) route: String,
    pub(crate) model: Option<String>,
    pub(crate) effort: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderProfileInput {
    pub(crate) project_id: String,
    pub(crate) participant_kind: String,
    pub(crate) route: String,
    pub(crate) model: Option<String>,
    pub(crate) effort: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    Chat,
    Build,
    Review,
    Revise,
    FinalReview,
}

impl Phase {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Build => "build",
            Self::Review => "review",
            Self::Revise => "revise",
            Self::FinalReview => "final-review",
        }
    }

    pub(crate) fn context_budget_bytes(self) -> usize {
        match self {
            Self::Chat => 8 * 1024,
            Self::Build => BUILD_CONTEXT_BUDGET_BYTES,
            Self::Review => 32 * 1024,
            Self::Revise => 16 * 1024,
            Self::FinalReview => 12 * 1024,
        }
    }
}

pub(crate) fn idle_timeout_seconds(_phase: Phase) -> u64 {
    PROCESS_IDLE_TIMEOUT_SECONDS
}

#[derive(Debug)]
pub(crate) struct ProviderRun {
    pub(crate) summary: String,
    pub(crate) session_id: Option<String>,
    pub(crate) success: bool,
    pub(crate) stopped: bool,
    pub(crate) timed_out: bool,
    pub(crate) idle_timed_out: bool,
    pub(crate) stderr: String,
    pub(crate) handoff: Option<AgentHandoff>,
    pub(crate) actual_model: Option<String>,
    pub(crate) usage: ProviderUsage,
    pub(crate) stdout_log_path: String,
    pub(crate) stderr_log_path: String,
    pub(crate) process_start_ms: u64,
    pub(crate) first_output_ms: Option<u64>,
    pub(crate) session_resumed: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct ChatSession {
    pub(crate) provider_session_id: String,
    pub(crate) last_seen_message_rowid: i64,
}

pub(crate) struct ChatReceiptMetrics {
    pub(crate) context_bytes: usize,
    pub(crate) preflight_ms: u64,
    pub(crate) total_ms: u64,
}

pub(crate) struct ReceiptMetrics {
    pub(crate) context_bytes: usize,
    pub(crate) packet_bytes_saved: usize,
    pub(crate) preflight_ms: u64,
    pub(crate) total_ms: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentHandoff {
    pub(crate) schema_version: u32,
    pub(crate) status: String,
    pub(crate) summary: String,
    #[serde(default)]
    pub(crate) changed_files: Vec<String>,
    #[serde(default)]
    pub(crate) checks: Vec<String>,
    #[serde(default)]
    pub(crate) findings: Vec<String>,
    pub(crate) next_action: String,
}

#[derive(Debug)]
pub(crate) struct SelectedSkill {
    pub(crate) relative_path: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) score: usize,
}
