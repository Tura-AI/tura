import MoreHorizontal from "lucide-solid/icons/ellipsis";
import FolderOpen from "lucide-solid/icons/folder-open";
import Pin from "lucide-solid/icons/pin";
import Plus from "lucide-solid/icons/plus";
import Settings from "lucide-solid/icons/settings";
import Trash2 from "lucide-solid/icons/trash-2";
import { Show, createSignal } from "solid-js";
import { t } from "../../i18n";

export function WorkspaceMenu(props: { onSettings: () => void; onNewSession: () => void }) {
  const [open, setOpen] = createSignal(false);
  return (
    <div class="workspace-menu">
      <button
        type="button"
        title={t("settings")}
        onClick={(event) => {
          event.stopPropagation();
          setOpen((value) => !value);
        }}
      >
        <MoreHorizontal size={15} strokeWidth={1.8} />
      </button>
      <Show when={open()}>
        <div class="rail-menu" onClick={(event) => event.stopPropagation()}>
          <button type="button">
            <Pin size={14} strokeWidth={1.7} />
            <span>{t("pinWorkspace")}</span>
          </button>
          <button type="button">
            <FolderOpen size={14} strokeWidth={1.7} />
            <span>{t("openInExplorer")}</span>
          </button>
          <button type="button" onClick={props.onNewSession}>
            <Plus size={14} strokeWidth={1.7} />
            <span>{t("newSession")}</span>
          </button>
          <button type="button" onClick={props.onSettings}>
            <Settings size={14} strokeWidth={1.7} />
            <span>{t("workspaceSettings")}</span>
          </button>
          <button type="button">
            <ArchiveIcon />
            <span>{t("archiveSession")}</span>
          </button>
          <button type="button">
            <Trash2 size={14} strokeWidth={1.7} />
            <span>{t("remove")}</span>
          </button>
        </div>
      </Show>
    </div>
  );
}

function ArchiveIcon() {
  return <span class="tiny-icon">▣</span>;
}
