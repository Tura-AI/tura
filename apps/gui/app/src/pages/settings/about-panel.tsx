import type { AboutOpenTarget, AboutUpdate } from "@tura/gateway-sdk";
import { createResource, createSignal, Show } from "solid-js";
import { useGlobalGateway } from "../../context/gateway";
import { t } from "../../i18n";

type AboutAction = "star" | "report" | "contribute" | "update" | "contact" | "install";

export function AboutPanel(props: { sessionId?: string }) {
  const { rootClient } = useGlobalGateway();
  const [info] = createResource(rootClient, (client) => client.aboutInfo());
  const [busy, setBusy] = createSignal<AboutAction>();
  const [notice, setNotice] = createSignal<string>();
  const [starred, setStarred] = createSignal(false);
  const [update, setUpdate] = createSignal<AboutUpdate>();

  async function addStar() {
    await run("star", async () => {
      const result = await rootClient().starTuraRepository();
      setStarred(result.outcome === "starred");
      setNotice(result.outcome === "starred" ? t("aboutStarred") : t("aboutStarOpened"));
    });
  }

  async function openTarget(action: AboutAction, target: AboutOpenTarget, message: string) {
    await run(action, async () => {
      await rootClient().openAboutTarget(target);
      setNotice(message);
    });
  }

  async function checkUpdate() {
    await run("update", async () => {
      const result = await rootClient().checkTuraUpdate();
      if (!result.update) {
        setNotice(t("aboutNoUpdate"));
        return;
      }
      setUpdate(result.update);
      setNotice(
        t("aboutUpdateAvailable", {
          version: result.update.latest_version,
        }),
      );
    });
  }

  async function installUpdate() {
    const pending = update();
    if (!pending) return;
    await run("install", async () => {
      const result = await rootClient().installTuraUpdate(pending.latest_version, props.sessionId);
      if (!result.scheduled) {
        throw new Error(t("aboutUpdateScheduleFailed"));
      }
      setUpdate(undefined);
      setNotice(t("aboutUpdateScheduled", { version: result.version }));
    });
  }

  async function run(action: AboutAction, operation: () => Promise<void>) {
    if (busy()) return;
    setBusy(action);
    setNotice(undefined);
    try {
      await operation();
    } catch (error) {
      setNotice(
        t("aboutActionFailed", {
          message: error instanceof Error ? error.message : String(error),
        }),
      );
    } finally {
      setBusy(undefined);
    }
  }

  return (
    <>
      <section class="settings-panel">
        <header>
          <span>{t("aboutRelease")}</span>
          <Show when={starred()}>
            <div class="settings-note">{t("aboutStarredStatus")}</div>
          </Show>
        </header>
        <Show
          when={info()}
          fallback={
            <div class="settings-fields">
              <div class="settings-note">{info.error ? t("aboutLoadFailed") : t("loading")}</div>
            </div>
          }
        >
          {(value) => (
            <div class="settings-fields">
              <div class="field-row readonly-row">
                <span>{t("aboutReleaseVersion")}</span>
                <code>{value().release_version}</code>
              </div>
              <div class="field-row readonly-row">
                <span>{t("aboutOperatingSystem")}</span>
                <code>{value().system.operating_system}</code>
              </div>
              <div class="field-row readonly-row">
                <span>{t("aboutOsVersion")}</span>
                <code>{value().system.os_version}</code>
              </div>
              <div class="field-row readonly-row">
                <span>{t("aboutArchitecture")}</span>
                <code>{value().system.architecture}</code>
              </div>
            </div>
          )}
        </Show>
      </section>

      <section class="settings-panel">
        <header>
          <span>{t("aboutSupportAndUpdates")}</span>
        </header>
        <div class="settings-list">
          <AboutActionButton
            label={t("aboutAddStar")}
            description={t("aboutAddStarDescription")}
            busy={busy() === "star"}
            disabled={Boolean(busy())}
            onClick={addStar}
          />
          <AboutActionButton
            label={t("aboutReportBug")}
            description={t("aboutReportBugDescription")}
            busy={busy() === "report"}
            disabled={Boolean(busy())}
            onClick={() => openTarget("report", "report_bug", t("aboutBugOpened"))}
          />
          <AboutActionButton
            label={t("aboutContribute")}
            description={t("aboutContributeDescription")}
            busy={busy() === "contribute"}
            disabled={Boolean(busy())}
            onClick={() => openTarget("contribute", "contribute", t("aboutContributeOpened"))}
          />
          <AboutActionButton
            label={t("aboutUpdate")}
            description={t("aboutUpdateDescription")}
            busy={busy() === "update"}
            disabled={Boolean(busy())}
            onClick={checkUpdate}
          />
          <AboutActionButton
            label={t("aboutContact")}
            description={t("aboutContactDescription")}
            busy={busy() === "contact"}
            disabled={Boolean(busy())}
            onClick={() => openTarget("contact", "contact", t("aboutContactOpened"))}
          />
        </div>
        <Show when={notice()}>{(message) => <div class="settings-note">{message()}</div>}</Show>
      </section>

      <Show when={update()}>
        {(pending) => (
          <div class="modal-scrim" onMouseDown={() => setUpdate(undefined)}>
            <div class="name-dialog" onMouseDown={(event) => event.stopPropagation()}>
              <header>
                <div>
                  <h2>{t("aboutUpdateTitle")}</h2>
                  <p>
                    {t("aboutUpdateWarning", {
                      current: pending().current_version,
                      latest: pending().latest_version,
                    })}
                  </p>
                </div>
                <button type="button" aria-label={t("close")} onClick={() => setUpdate(undefined)}>
                  &times;
                </button>
              </header>
              <footer>
                <button type="button" class="secondary" onClick={() => setUpdate(undefined)}>
                  {t("cancel")}
                </button>
                <button
                  type="button"
                  class="primary"
                  disabled={busy() === "install"}
                  onClick={installUpdate}
                >
                  {busy() === "install" ? t("aboutUpdating") : t("aboutUpdateNow")}
                </button>
              </footer>
            </div>
          </div>
        )}
      </Show>
    </>
  );
}

function AboutActionButton(props: {
  label: string;
  description: string;
  busy: boolean;
  disabled: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      class="settings-provider-row"
      disabled={props.disabled}
      aria-busy={props.busy}
      onClick={props.onClick}
    >
      <span>{props.label}</span>
      <small>{props.busy ? t("loading") : props.description}</small>
    </button>
  );
}
