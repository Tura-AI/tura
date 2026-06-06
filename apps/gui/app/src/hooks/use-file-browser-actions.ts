import {
  errorMessage,
  type FileContentResponse,
  type FileInfo,
  type GatewayClient,
} from "@tura/gateway-sdk";
import { createEffect, createSignal, type Accessor, type Setter } from "solid-js";
import { fixtureFileContent, fixtureFiles } from "../test/fixtures/app-fixtures";
import type { AppState } from "../state/global-store";
import { safe } from "../utils/safe";

export function useFileBrowserActions(options: {
  state: Accessor<AppState>;
  setState: Setter<AppState>;
  directoryClient: Accessor<GatewayClient>;
  e2eFixture?: string;
}) {
  const { state, setState, directoryClient, e2eFixture } = options;
  const [fileTree, setFileTree] = createSignal<Record<string, FileInfo[]>>({});
  const [fileLoadingPath, setFileLoadingPath] = createSignal<string>();
  const [fileContentLoadingPath, setFileContentLoadingPath] = createSignal<string>();
  const [expandedFileTreePaths, setExpandedFileTreePaths] = createSignal(new Set<string>());
  let fileContentRequestId = 0;

  async function readFiles(path = "") {
    return e2eFixture
      ? fixtureFiles(e2eFixture, path)
      : await safe(() => directoryClient().files(path), []);
  }

  async function loadFiles(path = "") {
    setFileLoadingPath(path);
    setFileContentLoadingPath(undefined);
    const files = await readFiles(path);
    setFileTree((previous) => ({ ...previous, [path]: files }));
    setState((previous) => ({
      ...previous,
      files,
      filePath: path,
      selectedFile: undefined,
      fileContent: undefined,
    }));
    setFileLoadingPath(undefined);
  }

  async function toggleFileTreeDirectory(file: FileInfo) {
    if (file.type !== "directory") {
      await openFile(file);
      return;
    }
    if (expandedFileTreePaths().has(file.path)) {
      setExpandedFileTreePaths((previous) => {
        const next = new Set(previous);
        next.delete(file.path);
        return next;
      });
      return;
    }
    setExpandedFileTreePaths((previous) => {
      const next = new Set(previous);
      next.add(file.path);
      return next;
    });
    setFileLoadingPath(file.path);
    const files = fileTree()[file.path] ?? (await readFiles(file.path));
    setFileTree((previous) => ({ ...previous, [file.path]: files }));
    setState((previous) => ({
      ...previous,
      files,
      filePath: file.path,
      selectedFile: undefined,
      fileContent: undefined,
    }));
    setFileContentLoadingPath(undefined);
    setFileLoadingPath(undefined);
  }

  createEffect(() => {
    if (
      state().activeTab === "files" &&
      state().files.length === 0 &&
      fileLoadingPath() === undefined &&
      fileTree()[""] === undefined
    ) {
      void loadFiles("");
    }
  });

  async function openFile(file: FileInfo) {
    if (file.type === "directory") {
      setExpandedFileTreePaths((previous) => {
        const next = new Set(previous);
        next.add(file.path);
        return next;
      });
      await loadFiles(file.path);
      return;
    }
    const requestId = ++fileContentRequestId;
    setFileContentLoadingPath(file.path);
    setState((previous) => ({
      ...previous,
      selectedFile: file,
      fileContent: undefined,
    }));
    const fileContent = e2eFixture
      ? fixtureFileContent(e2eFixture, file.path)
      : await safe(
          () => directoryClient().fileContent(file.path),
          undefined as FileContentResponse | undefined,
        );
    if (requestId !== fileContentRequestId) {
      return;
    }
    setFileContentLoadingPath(undefined);
    setState((previous) =>
      previous.selectedFile?.path === file.path ? { ...previous, fileContent } : previous,
    );
  }

  async function openSelectedFile() {
    const file = state().selectedFile;
    if (!file || e2eFixture) {
      return;
    }
    if (state().connection !== "connected") {
      setState((previous) => ({
        ...previous,
        error: "Gateway 未连接，无法调用系统默认应用打开文件。",
      }));
      return;
    }
    try {
      await directoryClient().openFile(file.path);
    } catch (error) {
      setState((previous) => ({ ...previous, error: errorMessage(error) }));
    }
  }

  async function openCurrentDirectory() {
    if (e2eFixture) {
      return;
    }
    if (state().connection !== "connected") {
      setState((previous) => ({
        ...previous,
        error: "Gateway 未连接，无法在系统文件浏览器中打开。",
      }));
      return;
    }
    try {
      const selected = state().selectedFile;
      await directoryClient().openFileLocation(selected?.path ?? state().filePath);
    } catch (error) {
      setState((previous) => ({ ...previous, error: errorMessage(error) }));
    }
  }

  return {
    fileTree,
    setFileTree,
    fileLoadingPath,
    fileContentLoadingPath,
    expandedFileTreePaths,
    loadFiles,
    openFile,
    toggleFileTreeDirectory,
    openCurrentDirectory,
    openSelectedFile,
  };
}
