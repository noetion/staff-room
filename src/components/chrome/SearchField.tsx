import { Search, X } from "lucide-react";
import type { RefObject } from "react";

export function SearchField({
  open,
  query,
  inputRef,
  onOpen,
  onClose,
  onQueryChange,
}: {
  open: boolean;
  query: string;
  inputRef: RefObject<HTMLInputElement | null>;
  onOpen: () => void;
  onClose: () => void;
  onQueryChange: (query: string) => void;
}) {
  return (
    <div className={`chrome-search ${open ? "is-open" : ""}`}>
      {open ? (
        <div className="chrome-search-field" role="search">
          <Search size={16} aria-hidden="true" />
          <input
            ref={inputRef}
            value={query}
            onChange={(event) => onQueryChange(event.target.value)}
            placeholder="Search rooms and evidence"
            aria-label="Search rooms and evidence"
          />
          <button type="button" className="chrome-search-close" onClick={onClose} aria-label="Close search">
            <X size={15} />
          </button>
        </div>
      ) : (
        <button type="button" className="chrome-search-button" onClick={onOpen}>
          <Search size={16} />
          <span>Search rooms and evidence</span>
          <kbd>Ctrl K</kbd>
        </button>
      )}
    </div>
  );
}
