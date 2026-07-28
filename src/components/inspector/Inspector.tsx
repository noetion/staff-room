import { useEffect, useRef } from "react";
import { X } from "lucide-react";
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
  const rootRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const root = rootRef.current;
    if (!root) return;
    const items = () => Array.from(root.querySelectorAll<HTMLElement>('button:not([disabled]), summary, [tabindex]:not([tabindex="-1"])'));
    items()[0]?.focus();
    const trap = (event: KeyboardEvent) => { if (event.key !== "Tab") return; const focusable = items(); const first = focusable[0]; const last = focusable[focusable.length - 1]; if (!first || !last) return; if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); } else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); } };
    root.addEventListener("keydown", trap);
    return () => root.removeEventListener("keydown", trap);
  }, []);
  return <><button type="button" className="inspector-scrim" aria-hidden="true" tabIndex={-1} onClick={onClose} /><Sheet open={true} onClose={onClose} title="Room context" className="inspector-sheet" role="dialog" aria-modal="true" aria-label="Room context"><div className="inspector-shell" id="room-context" ref={rootRef}><button type="button" className="inspector-close" aria-label="Close context" onClick={onClose}><X size={18} /></button><div className="inspector-tabs" role="tablist">{tabs.map((tab) => <button type="button" key={tab} id={`inspector-tab-${tab.toLowerCase()}`} className={activeTab === tab ? "active" : ""} onClick={() => setActiveTab(tab)} role="tab" aria-selected={activeTab === tab} aria-controls={`inspector-panel-${tab.toLowerCase()}`} tabIndex={activeTab === tab ? 0 : -1}>{tab}</button>)}</div><div className="inspector-body" id={`inspector-panel-${activeTab.toLowerCase()}`} role="tabpanel" aria-labelledby={`inspector-tab-${activeTab.toLowerCase()}`}>{activeTab === "Repository" && <RepositoryTab project={project} environment={environment} run={run} />}{activeTab === "Participants" && <ParticipantsTab environment={environment} />}{activeTab === "Evidence" && <EvidenceTab environment={environment} run={run} messages={messages} receipts={receipts} />}{activeTab === "Memory" && <MemoryTab run={run} />}</div></div></Sheet></>;
}
