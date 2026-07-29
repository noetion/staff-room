import { Monitor, Moon, Sun } from "lucide-react";
import { useEffect, useState } from "react";

type ThemePreference = "system" | "light" | "dark";

function themePreference(): ThemePreference {
  const preference = localStorage.getItem("ar-theme");
  return preference === "light" || preference === "dark" || preference === "system"
    ? preference
    : "system";
}

function applyTheme(preference: ThemePreference) {
  localStorage.setItem("ar-theme", preference);
  // "System" must leave the attribute off. Baking the resolved value in pinned
  // the app to whatever the OS happened to be at the moment of the click, and
  // disagreed with the boot script in main.tsx, which clears it.
  if (preference === "system") delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = preference;
}

export function ThemeToggle() {
  const [preference, setPreference] = useState<ThemePreference>(themePreference);

  // Keep the meta theme-color and any JS-observed state honest when the OS
  // flips while "system" is selected.
  useEffect(() => {
    if (preference !== "system" || typeof window.matchMedia !== "function") return;
    const query = window.matchMedia("(prefers-color-scheme: dark)");
    const sync = () => applyTheme("system");
    query.addEventListener("change", sync);
    return () => query.removeEventListener("change", sync);
  }, [preference]);
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
