import { RefreshCw } from "lucide-react";
import type { AgentKind, NativeEnvironment, ProjectSettings, ProviderProfile, VerificationConfig } from "../../model";
import type { ProviderDraft } from "../../lib/provider-profiles";
import type { VoiceStatus } from "../../native";
import { AutonomySetting } from "./AutonomySetting";
import { ProviderProfileCard } from "./ProviderProfileCard";
import { VerificationSettings } from "./VerificationSettings";
import { VoiceSettings } from "./VoiceSettings";
import "./settings.css";

export function SettingsView({ environment, profiles, projectSettings, verificationConfig, voiceStatus, savingProjectSettings, savingKind, refreshing, native, error, onRefresh, onSaveProfile, testingKind, onTestConnection, modelCatalog, discoveringModelsKind, modelDiscoveryDetails, onRefreshModels, onAutonomousShipChange, onPickVoiceEngine, onPickVoiceModel, onVerificationConfigChange, onSaveVerificationConfig }: { environment: NativeEnvironment; profiles: ProviderProfile[]; projectSettings: ProjectSettings; verificationConfig: VerificationConfig; voiceStatus: VoiceStatus; savingProjectSettings: boolean; savingKind?: AgentKind; refreshing: boolean; native: boolean; error: string; onRefresh: () => void; onSaveProfile: (profiles: ProviderProfile[]) => Promise<void>; testingKind?: AgentKind; onTestConnection: (kind: AgentKind, draft: ProviderDraft) => void; modelCatalog: Partial<Record<AgentKind, string[]>>; discoveringModelsKind?: AgentKind; modelDiscoveryDetails: Partial<Record<AgentKind, string>>; onRefreshModels: (kind: AgentKind) => void; onAutonomousShipChange: (enabled: boolean) => void; onPickVoiceEngine: () => void; onPickVoiceModel: () => void; onVerificationConfigChange: (config: VerificationConfig) => void; onSaveVerificationConfig: () => void }) {
  return <section className="utility-screen settings-view" aria-labelledby="settings-title">
    <header className="settings-header"><div><span className="eyebrow">Local runtime</span><h1 id="settings-title">Models and <span>runtime</span></h1><p>Choose exact provider models once per project. Agent Room stores the requested choice and records what each CLI reports for every phase.</p></div><button type="button" className="secondary-button" onClick={onRefresh} disabled={refreshing}><RefreshCw size={15} className={refreshing ? "spinning" : undefined} />{refreshing ? "Checking providers" : "Recheck providers"}</button></header>
    {!native && <p className="empty-state">Provider checks are available in the Tauri desktop app.</p>}
    {error && <p className="utility-error" role="alert">{error}</p>}
    <section className="settings-section"><span className="eyebrow">Autonomy</span><AutonomySetting enabled={projectSettings.autonomousShipEnabled} disabled onChange={onAutonomousShipChange} /></section>
    <section className="settings-section"><span className="eyebrow">Voice</span><VoiceSettings status={voiceStatus} disabled={!native} onPickEngine={onPickVoiceEngine} onPickModel={onPickVoiceModel} /></section>
    <section className="settings-section"><span className="eyebrow">Verification</span><VerificationSettings config={verificationConfig} disabled={!native || savingProjectSettings} onChange={onVerificationConfigChange} onSave={onSaveVerificationConfig} /></section>
    <section className="settings-section"><span className="eyebrow">Providers</span><div className="settings-grid">{environment.participants.map((participant) => <ProviderProfileCard key={participant.kind} participant={participant} profiles={profiles.filter((profile) => profile.participantKind === participant.kind)} saving={savingKind === participant.kind} onSave={onSaveProfile} testing={testingKind === participant.kind} onTest={onTestConnection} models={modelCatalog[participant.kind] ?? participant.models} hasAuthoritativeCatalog={modelCatalog[participant.kind] !== undefined} discoveringModels={discoveringModelsKind === participant.kind} modelDiscoveryDetail={modelDiscoveryDetails[participant.kind]} onRefreshModels={onRefreshModels} />)}</div></section>
    <p className="settings-disclosure">Token totals appear only when a CLI emits them in its native run output. Provider account quotas and reset windows are not scraped or guessed.</p>
  </section>;
}
