import {
  type ChangeEvent,
  type FormEvent,
  type KeyboardEvent,
  type ReactNode,
  useEffect,
  useId,
  useRef,
  useState,
} from "react";
import type { Participant } from "../../model";
import { Disclosure } from "../primitives";
import { MentionList } from "./MentionList";
import { RouteSwitch, type ComposerRoute } from "./RouteSwitch";
import { SendButton } from "./SendButton";
import "./composer.css";

type ComposerProps = {
  busy: boolean;
  canSelectParticipant: (participant: Participant) => boolean;
  contextualLabel: string;
  error: string;
  inputDescribedBy?: string;
  mode: ComposerRoute;
  onModeChange: (mode: ComposerRoute) => void;
  onObjectiveChange: (value: string) => void;
  onParticipantSelect: (participant: Participant) => void;
  onStop: () => void;
  onSubmit: (event: FormEvent) => void;
  participants: Participant[];
  placeholder: string;
  promoting: boolean;
  quickEditDisabled: boolean;
  quickEditPreview?: ReactNode;
  renderParticipantMark: (participant: Participant) => ReactNode;
  shipDisabled: boolean;
  sideChatAvailable: boolean;
  titleForParticipant: (participant: Participant) => string;
  value: string;
};

export function Composer({
  busy,
  canSelectParticipant,
  contextualLabel,
  error,
  inputDescribedBy,
  mode,
  onModeChange,
  onObjectiveChange,
  onParticipantSelect,
  onStop,
  onSubmit,
  participants,
  placeholder,
  promoting,
  quickEditDisabled,
  quickEditPreview,
  renderParticipantMark,
  shipDisabled,
  sideChatAvailable,
  titleForParticipant,
  value,
}: ComposerProps) {
  const shellRef = useRef<HTMLFormElement>(null);
  const mentionId = useId();
  const [mentionsOpen, setMentionsOpen] = useState(false);
  const [activeMention, setActiveMention] = useState(0);
  const mentionParticipants = participants.filter((participant) => mode === "ship" || participant.kind !== "antigravity");

  useEffect(() => {
    const shell = shellRef.current;
    const room = shell?.closest(".room");
    if (!shell || !(room instanceof HTMLElement)) return;
    const observer = new ResizeObserver(([entry]) => {
      room.style.setProperty("--composer-height", `${entry.borderBoxSize[0]?.blockSize ?? entry.contentRect.height}px`);
    });
    observer.observe(shell);
    return () => {
      observer.disconnect();
      room.style.removeProperty("--composer-height");
    };
  }, []);

  useEffect(() => {
    setActiveMention((current) => Math.min(current, Math.max(mentionParticipants.length - 1, 0)));
  }, [mentionParticipants.length]);

  const chooseParticipant = (participant: Participant) => {
    onParticipantSelect(participant);
    setMentionsOpen(false);
  };

  const handleChange = (event: ChangeEvent<HTMLTextAreaElement>) => {
    onObjectiveChange(event.target.value);
    const token = event.target.value.match(/(?:^|\s)@(\w*)$/);
    setMentionsOpen(Boolean(token));
    setActiveMention(0);
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (mentionsOpen && mentionParticipants.length) {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setActiveMention((current) => (current + 1) % mentionParticipants.length);
        return;
      }
      if (event.key === "ArrowUp") {
        event.preventDefault();
        setActiveMention((current) => (current - 1 + mentionParticipants.length) % mentionParticipants.length);
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        setMentionsOpen(false);
        return;
      }
      if (event.key === "Enter" && !event.shiftKey) {
        event.preventDefault();
        const participant = mentionParticipants[activeMention];
        if (participant && canSelectParticipant(participant)) chooseParticipant(participant);
        return;
      }
    }
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      event.currentTarget.form?.requestSubmit();
    }
  };

  const activeDescendant = mentionsOpen && mentionParticipants[activeMention]
    ? `${mentionId}-${mentionParticipants[activeMention].kind}`
    : undefined;

  return (
    <form ref={shellRef} className="composer glass glass--refract" onSubmit={onSubmit}>
      <div className="composer-top">
        <RouteSwitch
          value={mode}
          onChange={onModeChange}
          quickEditDisabled={quickEditDisabled}
          shipDisabled={shipDisabled}
        />
        {quickEditPreview && <Disclosure className="quick-edit-preview">{quickEditPreview}</Disclosure>}
        <span className="composer-contextual-label">{contextualLabel}</span>
      </div>
      <div className="composer-input-wrap">
        {mentionsOpen && (
          <MentionList
            activeIndex={activeMention}
            id={mentionId}
            onChoose={chooseParticipant}
            participants={mentionParticipants}
            renderMark={renderParticipantMark}
            selectable={canSelectParticipant}
            titleFor={titleForParticipant}
          />
        )}
        <textarea
          id="room-objective"
          value={value}
          onChange={handleChange}
          onKeyDown={handleKeyDown}
          placeholder={placeholder}
          aria-describedby={inputDescribedBy}
          aria-controls={mentionsOpen ? mentionId : undefined}
          aria-activedescendant={activeDescendant}
          rows={1}
        />
      </div>
      <div className="composer-bottom">
        <div className="composer-targets" aria-label="Participant targets">
          {mentionParticipants.map((participant) => (
            <button
              type="button"
              key={participant.kind}
              disabled={!canSelectParticipant(participant)}
              onClick={() => chooseParticipant(participant)}
              title={titleForParticipant(participant)}
            >
              {renderParticipantMark(participant)}
              <span>{participant.name}</span>
            </button>
          ))}
        </div>
        <SendButton
          busy={busy}
          disabled={!value.trim() || (mode === "ship" && promoting)}
          mode={mode}
          onStop={onStop}
          sideChatAvailable={sideChatAvailable}
          promoting={promoting}
        />
      </div>
      {error && <p className="composer-error" id="composer-error" role="alert">{error}</p>}
    </form>
  );
}
