import { Bot, GitBranch } from "lucide-react";
import type { Project } from "../../model";

type RoomHeaderProps = {
  project: Project;
  installedCount: number;
};

export function RoomHeader({ project, installedCount }: RoomHeaderProps) {
  return (
    <header className="room-header">
      <h1>{project.name}</h1>
      <p title={project.goal}>{project.goal}</p>
      <div className="room-meta" aria-label="Project details">
        <span className="room-chip"><GitBranch size={14} aria-hidden="true" />{project.branch}</span>
        <span className="room-chip"><Bot size={14} aria-hidden="true" />{installedCount} of 4 ready</span>
      </div>
    </header>
  );
}
