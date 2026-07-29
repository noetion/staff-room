import { ChevronRight } from "lucide-react";
import type { Project } from "../../model";
import { Button, Monogram } from "../primitives";

type AttachProjectViewProps = {
  projects: Project[];
  native: boolean;
  onAttach: () => void;
  onSelect: (id: string) => void;
};

export function AttachProjectView({ projects, native, onAttach, onSelect }: AttachProjectViewProps) {
  return (
    <section className="utility-screen attach-project" aria-labelledby="attach-project-title">
      <span className="ghost-wordmark" aria-hidden="true">AGENT ROOM</span>
      <div className="attach-project-content">
        <header className="attach-project-copy">
          <h1 id="attach-project-title">
            Point a coding agent at a repository. <span>Watch it work.</span>
          </h1>
          <p>Attach a Git repository to create an independently scoped Agent Room.</p>
        </header>
        <Button type="button" className="attach-project-action" onClick={onAttach} disabled={!native}>
          Choose a repository <ChevronRight size={16} aria-hidden="true" />
        </Button>
        {!native && <p className="preview-notice">Native repository attachment is unavailable in browser preview.</p>}
        {projects.length > 0 && (
          <div className="recent-projects">
            <span className="recent-projects-label">Recent projects</span>
            {projects.map((recentProject) => (
              <button key={recentProject.id} type="button" className="recent-project" onClick={() => onSelect(recentProject.id)}>
                {/* Monogram renders its `label`, not its children, so the label
                    has to be the two-letter mark rather than the full name. */}
                <Monogram label={recentProject.name.slice(0, 2).toUpperCase()} aria-hidden="true" />
                <span className="recent-project-copy">
                  <strong>{recentProject.name}</strong>
                  <code>{recentProject.branch}</code>
                </span>
                <ChevronRight size={16} aria-hidden="true" />
              </button>
            ))}
          </div>
        )}
      </div>
    </section>
  );
}
