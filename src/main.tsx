import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";
import { installMock, isTauri } from "./lib/mock";
import { useApp } from "./stores/app";

if (!isTauri()) {
  installMock();
}

const s = useApp.getState();
void s.loadSettings();
void s.refreshInstances();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
