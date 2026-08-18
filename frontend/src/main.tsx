import React from "react";
import ReactDOM from "react-dom/client";
import { repository } from "./lib/repository";
import "./styles/global.css";

const root = document.getElementById("root");
if (!root) throw new Error("Root element #root not found");

await repository.hydrate();
const { App } = await import("./App");

ReactDOM.createRoot(root).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
