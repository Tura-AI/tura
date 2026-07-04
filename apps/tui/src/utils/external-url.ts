import { spawn } from "node:child_process";

export interface OpenExternalUrlResult {
  ok: boolean;
  reason?: string;
}

export type ExternalUrlOpener = (url: string) => Promise<OpenExternalUrlResult>;

let opener: ExternalUrlOpener = defaultOpenExternalUrl;

export async function openExternalUrl(url: string): Promise<OpenExternalUrlResult> {
  return opener(url);
}

export function setExternalUrlOpenerForTests(next?: ExternalUrlOpener): void {
  opener = next ?? defaultOpenExternalUrl;
}

async function defaultOpenExternalUrl(url: string): Promise<OpenExternalUrlResult> {
  if (!isSupportedExternalUrl(url)) {
    return { ok: false, reason: "Only http(s) URLs can be opened automatically." };
  }
  const [command, args] = externalOpenCommand(url);
  try {
    const child = spawn(command, args, { detached: true, stdio: "ignore", windowsHide: true });
    child.unref();
    return { ok: true };
  } catch (error) {
    return {
      ok: false,
      reason: error instanceof Error ? error.message : String(error),
    };
  }
}

function isSupportedExternalUrl(value: string): boolean {
  try {
    const parsed = new URL(value);
    return parsed.protocol === "http:" || parsed.protocol === "https:";
  } catch {
    return false;
  }
}

function externalOpenCommand(url: string): [string, string[]] {
  if (process.platform === "win32") {
    return ["rundll32.exe", ["url.dll,FileProtocolHandler", url]];
  }
  if (process.platform === "darwin") {
    return ["open", [url]];
  }
  return ["xdg-open", [url]];
}
