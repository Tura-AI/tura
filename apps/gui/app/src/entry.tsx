import { render } from "solid-js/web";
import { Router } from "@solidjs/router";
import { t } from "./i18n";
import "./styles/index.css";

const root = document.getElementById("root");

if (!root) {
  throw new Error(t("missingRootElement"));
}

try {
  const { App } = await import("./app");
  const { AppRoutes } = await import("./routes/app-routes");
  render(
    () => (
      <Router>
        <AppRoutes component={App} />
      </Router>
    ),
    root,
  );
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  root.textContent = t("startupFailed", { message });
  throw error;
}
