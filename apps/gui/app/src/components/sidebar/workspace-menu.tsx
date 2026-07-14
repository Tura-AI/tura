import MoreHorizontal from "lucide-solid/icons/ellipsis";
import Trash2 from "lucide-solid/icons/trash-2";
import { Show, createSignal } from "solid-js";
import { t } from "../../i18n";

export function WorkspaceMenu(props: { onDeleteWorkspace: () => void }) {
  const [open, setOpen] = createSignal(false);

  return (
    <div class="workspace-menu">
      <button
        type="button"
        title={t("deleteWorkspace")}
        onClick={(event) => {
          event.stopPropagation();
          setOpen((value) => !value);
        }}
      >
        <MoreHorizontal size={15} strokeWidth={1.8} />
      </button>
      <Show when={open()}>
        <div class="rail-menu" onClick={(event) => event.stopPropagation()}>
          <button
            type="button"
            onClick={(event) => {
              event.stopPropagation();
              setOpen(false);
              props.onDeleteWorkspace();
            }}
          >
            <Trash2 size={14} strokeWidth={1.7} />
            <span>{t("deleteWorkspace")}</span>
          </button>
        </div>
      </Show>
    </div>
  );
}
