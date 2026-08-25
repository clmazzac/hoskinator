import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "./index.css";
import ResumeEditor from "./components/ResumeEditor";

const root = document.getElementById("root");
if (!root) {
  throw new Error("index.html is missing its root element");
}

createRoot(root).render(
  <StrictMode>
    <ResumeEditor />
  </StrictMode>,
);
