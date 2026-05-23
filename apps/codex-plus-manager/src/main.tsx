import { createRoot } from "react-dom/client";
import { App } from "./App";
import { bootstrapInitialTheme } from "./state/useTheme";
import "./styles.css";

bootstrapInitialTheme();

const app = document.getElementById("app");

if (app instanceof HTMLElement) {
  createRoot(app).render(<App />);
}
