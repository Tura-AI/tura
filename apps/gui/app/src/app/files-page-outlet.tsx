import type { Setter } from "solid-js";
import { FileBrowserView } from "../pages/files/file-browser";
import type { AppState } from "../state/global-store";
import { parentPath } from "../utils/app-format";
import type { AppShellViewModel } from "./app-shell-view-model";

export function FilesPageOutlet(props: {
  state: AppState;
  setState: Setter<AppState>;
  view: Pick<
    AppShellViewModel,
    | "fileContentLoadingPath"
    | "loadFiles"
    | "openCurrentDirectory"
    | "openFile"
    | "openSelectedFile"
  >;
}) {
  const { fileContentLoadingPath, loadFiles, openCurrentDirectory, openFile, openSelectedFile } =
    props.view;

  return (
    <FileBrowserView
      path={props.state.filePath}
      directory={props.state.directory}
      files={props.state.files}
      selectedFile={props.state.selectedFile}
      fileContent={props.state.fileContent}
      fileContentLoadingPath={fileContentLoadingPath()}
      onFile={openFile}
      onUp={() => loadFiles(parentPath(props.state.filePath))}
      onOpenDirectory={openCurrentDirectory}
      onList={() =>
        props.setState((previous) => ({
          ...previous,
          selectedFile: undefined,
          fileContent: undefined,
        }))
      }
      onOpenExternal={openSelectedFile}
    />
  );
}
