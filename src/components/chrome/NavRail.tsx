import { Activity, Settings, TerminalSquare } from "lucide-react";
import type { Project } from "../../model";

type PrimaryView = "rooms" | "activity" | "settings";

export function NavRail({
  activeView,
  onNavigate,
  projects,
  activeProjectId,
  onSelectProject,
  onAttachProject,
}: {
  activeView: PrimaryView;
  onNavigate: (view: PrimaryView) => void;
  projects: Project[];
  activeProjectId: string;
  onSelectProject: (id: string) => void;
  onAttachProject: () => void;
}) {
  return (
    <aside className="chrome-nav-rail glass glass--refract">
      <nav className="chrome-primary-nav" aria-label="Primary">
        <button type="button" className={`chrome-nav-button ${activeView === "rooms" ? "active" : ""}`} aria-current={activeView === "rooms" ? "page" : undefined} onClick={() => onNavigate("rooms")}>
          <TerminalSquare size={19} />
          <span>Rooms</span>
        </button>
        <button type="button" className={`chrome-nav-button ${activeView === "activity" ? "active" : ""}`} aria-current={activeView === "activity" ? "page" : undefined} onClick={() => onNavigate("activity")}>
          <Activity size={19} />
          <span>Activity</span>
        </button>
        <button type="button" className={`chrome-nav-button ${activeView === "settings" ? "active" : ""}`} aria-current={activeView === "settings" ? "page" : undefined} onClick={() => onNavigate("settings")}>
          <Settings size={19} />
          <span>Settings</span>
        </button>
      </nav>
      <div className="chrome-rail-projects" aria-label="Projects">
        {projects.map((project) => (
          <button key={project.id} type="button" className={`chrome-rail-project ${project.id === activeProjectId ? "active" : ""}`} onClick={() => onSelectProject(project.id)} title={project.repositoryPath}>
            {project.name.slice(0, 2).toUpperCase()}
          </button>
        ))}
        <button type="button" className="chrome-rail-project chrome-rail-project-add" onClick={onAttachProject} aria-label="Attach repository">
          +
        </button>
      </div>
      <div className="chrome-rail-foot">
        <span className="chrome-local-indicator" />
        <span>Local</span>
      </div>
    </aside>
  );
}
