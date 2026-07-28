import "@fontsource-variable/onest";
import "@fontsource/ibm-plex-sans/400.css";
import "@fontsource/ibm-plex-sans/500.css";
import "@fontsource/ibm-plex-sans/600.css";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/500.css";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./design/index.css";

const theme = localStorage.getItem("ar-theme");

if (theme === "light" || theme === "dark") {
  document.documentElement.dataset.theme = theme;
} else {
  delete document.documentElement.dataset.theme;
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
