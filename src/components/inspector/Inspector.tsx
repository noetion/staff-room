import { Sheet } from "../primitives";
import type { ExecutionReceipt, NativeEnvironment, Project, RoomMessage, Run } from "../../model";
import { RepositoryTab } from "./RepositoryTab";
import { ParticipantsTab } from "./ParticipantsTab";
import { EvidenceTab } from "./EvidenceTab";
import { MemoryTab } from "./MemoryTab";

type InspectorTab = "Repository" | "Participants" | "Evidence" | "Memory";
type Props = { project: Project; environment: NativeEnvironment; activeTab: InspectorTab; setActiveTab: (tab: InspectorTab) => void; run: Run; messages: RoomMessage[]; receipts: ExecutionReceipt[]; onClose: () => void };
const tabs: InspectorTab[] = ["Repository", "Participants", "Evidence", "Memory"];

export function Inspector({ project, environment, activeTab, setActiveTab, run, messages, receipts, onClose }: Props) {
  return <Sheet open={true} onClose={onClose} title="Room context" className="inspector-sheet"><div className="inspector-shell" id="room-context"><div className="inspector-tabs" role="tablist">{tabs.map((tab) => <button type="button" key={tab} id={`inspector-tab-${tab.toLowerCase()}`} className={activeTab === tab ? "active" : ""} onClick={() => setActiveTab(tab)} role="tab" aria-selected={activeTab === tab} aria-controls={activeTab === tab ? `inspector-panel-${tab.toLowerCase()}` : undefined} tabIndex={activeTab === tab ? 0 : -1}>{tab}</button>)}</div><div className="inspector-body" id={`inspector-panel-${activeTab.toLowerCase()}`} role="tabpanel" aria-labelledby={`inspector-tab-${activeTab.toLowerCase()}`}>{activeTab === "Repository" && <RepositoryTab project={project} environment={environment} run={run} />}{activeTab === "Participants" && <ParticipantsTab environment={environment} />}{activeTab === "Evidence" && <EvidenceTab environment={environment} run={run} messages={messages} receipts={receipts} />}{activeTab === "Memory" && <MemoryTab run={run} />}</div></div></Sheet>;
}
