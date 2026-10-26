import React from "react";
import ReactDOM from "react-dom/client";
import Picker from "./Picker";
import Settings from "./Settings";
import "./App.css";

const isSettings = window.location.hash === "#settings";
document.documentElement.dataset.view = isSettings ? "settings" : "picker";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{isSettings ? <Settings /> : <Picker />}</React.StrictMode>,
);
