import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { EditorApp } from "./EditorApp";
import "./styles/global.css";
import "./components/ui/Button.css";
import "./components/ui/Input.css";
import "./components/ui/Badge.css";
import "./components/player/VideoPlayer.css";
import "./components/editor/editor.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <EditorApp />
  </StrictMode>,
);
