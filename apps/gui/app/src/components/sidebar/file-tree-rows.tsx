import type { FileInfo } from "@tura/gateway-sdk";
import { For, Show } from "solid-js";
import { t } from "../../i18n";
import { FileTreeLabel } from "../../pages/new-session";
import { classNames } from "../../state/format";

export function FileTreeRows(props: {
  files: FileInfo[];
  fileTree: Record<string, FileInfo[]>;
  activePath: string;
  loadingPath?: string;
  expandedPaths: Set<string>;
  selectedFile?: FileInfo;
  depth: number;
  onFile: (file: FileInfo) => void;
  onDirectory: (file: FileInfo) => void;
}) {
  return (
    <For
      each={props.files}
      fallback={props.depth === 1 ? <div class="rail-empty">{t("empty")}</div> : null}
    >
      {(file) => {
        const loadedChildren = () => props.fileTree[file.path] ?? [];
        const expanded = () => file.type === "directory" && props.expandedPaths.has(file.path);
        return (
          <>
            <button
              class={classNames(
                "child-row",
                "file-tree-row",
                file.type === "directory" && "tree-folder",
                props.selectedFile?.path === file.path && "selected",
              )}
              style={{ "--depth": 1, "--file-depth": props.depth }}
              onClick={() =>
                file.type === "directory" ? props.onDirectory(file) : props.onFile(file)
              }
              title={file.path}
            >
              <FileTreeLabel file={file} expanded={expanded()} />
              <Show when={props.loadingPath === file.path}>
                <span class="file-tree-loading loading-bar" />
              </Show>
            </button>
            <Show when={expanded()}>
              <FileTreeRows
                files={loadedChildren()}
                fileTree={props.fileTree}
                activePath={props.activePath}
                loadingPath={props.loadingPath}
                expandedPaths={props.expandedPaths}
                selectedFile={props.selectedFile}
                depth={props.depth + 1}
                onFile={props.onFile}
                onDirectory={props.onDirectory}
              />
            </Show>
          </>
        );
      }}
    </For>
  );
}
