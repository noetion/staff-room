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
import { Mic } from "lucide-react";
import type { Participant } from "../../model";
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
  onVoiceError: (error: string) => void;
  onVoiceStart: () => Promise<void>;
  onVoiceStop: (cancel?: boolean) => Promise<string>;
  subscribeVoiceLevel: (handler: (level: number) => void) => Promise<() => void>;
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
  voiceAvailable: boolean;
  voiceDetail: string;
  voiceDisabled: boolean;
};

export function insertTranscript(
  value: string,
  start: number,
  end: number,
  transcript: string,
): { value: string; caret: number } {
  const text = transcript.trim();
  if (!text) return { value, caret: start };
  const before = value.slice(0, start);
  const after = value.slice(end);
  const prefix = before && !/\s$/.test(before) ? " " : "";
  const suffix = after && !/^\s/.test(after) ? " " : "";
  const inserted = `${prefix}${text}${suffix}`;
  return {
    value: `${before}${inserted}${after}`,
    caret: before.length + prefix.length + text.length,
  };
}

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
  onVoiceError,
  onVoiceStart,
  onVoiceStop,
  subscribeVoiceLevel,
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
  voiceAvailable,
  voiceDetail,
  voiceDisabled,
}: ComposerProps) {
  const shellRef = useRef<HTMLFormElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const meterRef = useRef<HTMLSpanElement>(null);
  const mentionId = useId();
  const [mentionsOpen, setMentionsOpen] = useState(false);
  const [activeMention, setActiveMention] = useState(0);
  const [voicePhase, setVoicePhase] = useState<"idle" | "starting" | "recording" | "transcribing">("idle");
  const voicePhaseRef = useRef(voicePhase);
  const voiceReleasedRef = useRef(false);
  const voiceSelectionRef = useRef({ start: 0, end: 0 });
  const valueRef = useRef(value);
  valueRef.current = value;
  voicePhaseRef.current = voicePhase;
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

  const finishVoice = async (cancel = false) => {
    if (voicePhaseRef.current === "starting") {
      voiceReleasedRef.current = true;
      return;
    }
    if (voicePhaseRef.current !== "recording") return;
    voicePhaseRef.current = "transcribing";
    setVoicePhase("transcribing");
    try {
      const transcript = await onVoiceStop(cancel);
      if (!cancel && transcript.trim()) {
        const { start, end } = voiceSelectionRef.current;
        const inserted = insertTranscript(valueRef.current, start, end, transcript);
        onObjectiveChange(inserted.value);
        requestAnimationFrame(() => {
          textareaRef.current?.focus();
          textareaRef.current?.setSelectionRange(inserted.caret, inserted.caret);
        });
      }
    } catch (error) {
      onVoiceError(error instanceof Error ? error.message : String(error));
    } finally {
      voicePhaseRef.current = "idle";
      setVoicePhase("idle");
    }
  };

  useEffect(() => {
    if (voicePhase !== "recording") return;
    const cancel = () => {
      if (voicePhaseRef.current === "recording") void finishVoice(true);
    };
    const onVisibility = () => {
      if (document.hidden) cancel();
    };
    window.addEventListener("blur", cancel);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      window.removeEventListener("blur", cancel);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [voicePhase]);

  useEffect(() => {
    const meter = meterRef.current;
    if (voicePhase !== "recording" || !meter) return;
    let dispose: (() => void) | undefined;
    let cancelled = false;
    void subscribeVoiceLevel((level) => {
      meter.style.setProperty("--voice-level", level.toFixed(3));
    })
      .then((off) => {
        if (cancelled) off();
        else dispose = off;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      dispose?.();
      meter.style.setProperty("--voice-level", "0");
    };
  }, [voicePhase, subscribeVoiceLevel]);

  const beginVoice = async () => {
    if (!voiceAvailable || voiceDisabled || voicePhaseRef.current !== "idle") return;
    const textarea = textareaRef.current;
    // An unfocused textarea reports a caret at 0, which would splice dictation in front
    // of whatever the person already typed. Append instead.
    const focused = textarea !== null && document.activeElement === textarea;
    voiceSelectionRef.current = focused
      ? { start: textarea.selectionStart, end: textarea.selectionEnd }
      : { start: valueRef.current.length, end: valueRef.current.length };
    voiceReleasedRef.current = false;
    voicePhaseRef.current = "starting";
    setVoicePhase("starting");
    try {
      await onVoiceStart();
      voicePhaseRef.current = "recording";
      setVoicePhase("recording");
      if (voiceReleasedRef.current) await finishVoice();
    } catch (error) {
      voicePhaseRef.current = "idle";
      setVoicePhase("idle");
      onVoiceError(error instanceof Error ? error.message : String(error));
    }
  };

  return (
    <form ref={shellRef} className="composer glass glass--refract" onSubmit={onSubmit}>
      <div className="composer-top">
        <RouteSwitch
          value={mode}
          onChange={onModeChange}
          quickEditDisabled={quickEditDisabled}
          shipDisabled={shipDisabled}
        />
        <span className="composer-contextual-label">{contextualLabel}</span>
      </div>
      {quickEditPreview && (
        <section className="quick-edit-preview" aria-label="Quick Edit review">
          {quickEditPreview}
        </section>
      )}
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
        <label className="visually-hidden" htmlFor="room-objective">
          {mode === "ship" ? "Engineering objective" : mode === "quick-edit" ? "Quick Edit instruction" : "Message"}
        </label>
        <textarea
          ref={textareaRef}
          id="room-objective"
          value={value}
          onChange={handleChange}
          onKeyDown={handleKeyDown}
          placeholder={placeholder}
          aria-describedby={inputDescribedBy}
          aria-controls={mentionsOpen ? mentionId : undefined}
          aria-activedescendant={activeDescendant}
          aria-autocomplete="list"
          aria-expanded={mentionsOpen}
          role="combobox"
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
        <button
          type="button"
          className="composer-voice"
          data-state={voicePhase}
          disabled={!voiceAvailable || voiceDisabled || voicePhase === "transcribing"}
          aria-label={voicePhase === "recording" ? "Release to transcribe" : "Hold to dictate"}
          aria-pressed={voicePhase === "recording"}
          title={voiceAvailable ? `Hold to dictate, up to 30 seconds. ${voiceDetail}` : voiceDetail}
          onPointerDown={(event) => {
            event.preventDefault();
            event.currentTarget.setPointerCapture(event.pointerId);
            void beginVoice();
          }}
          onPointerUp={() => void finishVoice()}
          onPointerCancel={() => void finishVoice(true)}
          onKeyDown={(event) => {
            if ((event.key === " " || event.key === "Enter") && !event.repeat) {
              event.preventDefault();
              void beginVoice();
            }
            if (event.key === "Escape") {
              event.preventDefault();
              void finishVoice(true);
            }
          }}
          onKeyUp={(event) => {
            if (event.key === " " || event.key === "Enter") {
              event.preventDefault();
              void finishVoice();
            }
          }}
        >
          <Mic aria-hidden="true" />
          <span>{voicePhase === "recording" ? "Listening" : voicePhase === "transcribing" ? "Transcribing" : "Dictate"}</span>
          <span className="composer-voice-meter" ref={meterRef} aria-hidden="true" />
        </button>
        <span className="visually-hidden" role="status" aria-live="polite">
          {voicePhase === "recording"
            ? "Listening. Release to transcribe."
            : voicePhase === "transcribing"
              ? "Transcribing your dictation."
              : ""}
        </span>
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
