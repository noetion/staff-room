import { ChevronRight, PanelRight } from "lucide-react";
import type { RefObject } from "react";
import type { Project } from "../../model";
import { SearchField } from "./SearchField";
import { ThemeToggle } from "./ThemeToggle";
import { WindowControls } from "./WindowControls";

export function TitleBar({
  project,
  native,
  autonomousShipEnabled,
  searchOpen,
  searchQuery,
  searchInputRef,
  inspectorOpen,
  contextToggleRef,
  onOpenRepository,
  onSearchOpen,
  onSearchClose,
  onSearchQueryChange,
  onInspectorToggle,
  onToggleMaximize,
}: {
  project: Project;
  native: boolean;
  autonomousShipEnabled: boolean;
  searchOpen: boolean;
  searchQuery: string;
  searchInputRef: RefObject<HTMLInputElement | null>;
  inspectorOpen: boolean;
  contextToggleRef: RefObject<HTMLButtonElement | null>;
  onOpenRepository: () => void;
  onSearchOpen: () => void;
  onSearchClose: () => void;
  onSearchQueryChange: (query: string) => void;
  onInspectorToggle: () => void;
  onToggleMaximize: () => void;
}) {
  const status = native
    ? autonomousShipEnabled ? "Autonomy armed" : "Local runtime"
    : "Read-only preview";

  return (
    <header className="chrome-titlebar glass glass--refract">
      <div className="chrome-brand">
        <span className="chrome-brand-mark"><span /></span>
        <strong>Agent Room</strong>
      </div>
      <button type="button" className="chrome-project-switch" onClick={onOpenRepository} aria-label={`Open ${project.name} repository details`}>
        <span className="chrome-project-monogram">AR</span>
        <span>
          <strong>{project.name}</strong>
          <small>{project.branch}</small>
        </span>
        <ChevronRight size={15} />
      </button>
      <div className="chrome-titlebar-drag" data-tauri-drag-region onDoubleClick={onToggleMaximize} aria-hidden="true" />
      <SearchField open={searchOpen} query={searchQuery} inputRef={searchInputRef} onOpen={onSearchOpen} onClose={onSearchClose} onQueryChange={onSearchQueryChange} />
      <div className="chrome-status" aria-label={status}>
        <span className={native ? "online" : ""} />
        {status}
      </div>
      <ThemeToggle />
      <button ref={contextToggleRef} type="button" className="chrome-inspector-toggle" aria-label={inspectorOpen ? "Close context" : "Open context"} aria-expanded={inspectorOpen} aria-controls={inspectorOpen ? "room-context" : undefined} onClick={onInspectorToggle}>
        <PanelRight size={18} />
      </button>
      <WindowControls native={native} />
    </header>
  );
}
