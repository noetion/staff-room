import { Chip } from "../primitives";
import type { ReactNode } from "react";
import type { Participant } from "../../model";

type MentionListProps = {
  activeIndex: number;
  id: string;
  onChoose: (participant: Participant) => void;
  participants: Participant[];
  renderMark: (participant: Participant) => ReactNode;
  selectable: (participant: Participant) => boolean;
  titleFor: (participant: Participant) => string;
};

export function MentionList({
  activeIndex,
  id,
  onChoose,
  participants,
  renderMark,
  selectable,
  titleFor,
}: MentionListProps) {
  return (
    <div className="mention-list glass glass--clear" id={id} role="listbox" aria-label="Choose a participant">
      {participants.map((participant, index) => {
        const disabled = !selectable(participant);
        const optionId = `${id}-${participant.kind}`;
        return (
          <button
            type="button"
            key={participant.kind}
            id={optionId}
            role="option"
            aria-selected={index === activeIndex}
            disabled={disabled}
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => onChoose(participant)}
            title={titleFor(participant)}
          >
            {renderMark(participant)}
            <span>{participant.name}</span>
            <Chip>{participant.capabilities.autonomyMode}</Chip>
          </button>
        );
      })}
    </div>
  );
}
