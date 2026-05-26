import { render } from "solid-js/web";
import { t } from "./i18n";
import "./styles/index.css";

const root = document.getElementById("root");

if (!root) {
  throw new Error(t("missingRootElement"));
}

try {
  const { App } = await import("./app");
  render(() => <App />, root);
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  root.textContent = t("startupFailed", { message });
  throw error;
}
