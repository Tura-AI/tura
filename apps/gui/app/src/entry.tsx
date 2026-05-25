import { render } from "solid-js/web";
import "./styles/index.css";

const root = document.getElementById("root");

if (!root) {
  throw new Error("Missing root element");
}

try {
  const { App } = await import("./app");
  render(() => <App />, root);
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  root.textContent = `Tura failed to start: ${message}`;
  throw error;
}
