import { Monitor, Moon, Sun } from "lucide-react";
import { useState } from "react";

type ThemePreference = "system" | "light" | "dark";

function themePreference(): ThemePreference {
  const preference = localStorage.getItem("ar-theme");
  return preference === "light" || preference === "dark" || preference === "system"
    ? preference
    : "system";
}

function applyTheme(preference: ThemePreference) {
  localStorage.setItem("ar-theme", preference);
  const resolved = preference === "system"
    ? window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light"
    : preference;
  document.documentElement.dataset.theme = resolved;
}

export function ThemeToggle() {
  const [preference, setPreference] = useState<ThemePreference>(themePreference);
  const nextPreference: Record<ThemePreference, ThemePreference> = {
    system: "light",
    light: "dark",
    dark: "system",
  };
  const label = preference === "system" ? "system theme" : `${preference} theme`;
  const Icon = preference === "system" ? Monitor : preference === "light" ? Sun : Moon;

  return (
    <button
      type="button"
      className="chrome-theme-toggle"
      aria-label={`Theme: ${label}. Activate to switch theme.`}
      onClick={() => {
        const next = nextPreference[preference];
        setPreference(next);
        applyTheme(next);
      }}
    >
      <Icon size={15} aria-hidden="true" />
    </button>
  );
}
