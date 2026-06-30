import { execFile } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import { basename, resolve, sep } from "node:path";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

export async function saveClipboardImageInput(cwd: string): Promise<string | undefined> {
  const image = await readClipboardImage();
  if (!image) return undefined;
  return saveInputBytes(cwd, image.name, image.bytes);
}

export async function saveInputBytes(
  cwd: string,
  requestedName: string,
  bytes: Buffer,
): Promise<string> {
  if (bytes.length === 0) throw new Error("clipboard image payload is empty");
  const directory = resolve(cwd, ".tura", "media", "input");
  await mkdir(directory, { recursive: true });
  const name = `${Date.now()}-${process.pid}-${sanitizeInputFileName(requestedName)}`;
  const absolute = resolve(directory, name);
  await writeFile(absolute, bytes);
  return relativeWorkspacePath(cwd, absolute);
}

export function mediaTokenForInputPath(path: string): string {
  return `[MEDIA:${path}:MEDIA]`;
}

async function readClipboardImage(): Promise<{ name: string; bytes: Buffer } | undefined> {
  if (process.env.TURA_TUI_CLIPBOARD_IMAGE_BASE64) {
    return {
      name: process.env.TURA_TUI_CLIPBOARD_IMAGE_NAME || "clipboard.png",
      bytes: Buffer.from(process.env.TURA_TUI_CLIPBOARD_IMAGE_BASE64, "base64"),
    };
  }
  if (process.platform === "win32") return readWindowsClipboardImage();
  if (process.platform === "darwin") return readMacClipboardImage();
  return readLinuxClipboardImage();
}

async function readWindowsClipboardImage(): Promise<{ name: string; bytes: Buffer } | undefined> {
  const script = [
    "Add-Type -AssemblyName System.Windows.Forms;",
    "$image=[System.Windows.Forms.Clipboard]::GetImage();",
    "if ($null -eq $image) { exit 3 }",
    "$stream=New-Object System.IO.MemoryStream;",
    "$image.Save($stream,[System.Drawing.Imaging.ImageFormat]::Png);",
    "[Console]::Out.Write([Convert]::ToBase64String($stream.ToArray()))",
  ].join(" ");
  const result = await runClipboardCommand("powershell.exe", [
    "-NoProfile",
    "-STA",
    "-Command",
    script,
  ]);
  return result ? { name: "clipboard.png", bytes: Buffer.from(result, "base64") } : undefined;
}

async function readMacClipboardImage(): Promise<{ name: string; bytes: Buffer } | undefined> {
  const result = await runClipboardCommand("pngpaste", ["-"]);
  return result ? { name: "clipboard.png", bytes: Buffer.from(result, "binary") } : undefined;
}

async function readLinuxClipboardImage(): Promise<{ name: string; bytes: Buffer } | undefined> {
  const wl = await runClipboardCommand("wl-paste", ["--type", "image/png", "--no-newline"]);
  if (wl) return { name: "clipboard.png", bytes: Buffer.from(wl, "binary") };
  const xclip = await runClipboardCommand("xclip", [
    "-selection",
    "clipboard",
    "-t",
    "image/png",
    "-o",
  ]);
  return xclip ? { name: "clipboard.png", bytes: Buffer.from(xclip, "binary") } : undefined;
}

async function runClipboardCommand(command: string, args: string[]): Promise<string | undefined> {
  try {
    const { stdout } = await execFileAsync(command, args, {
      encoding: "binary",
      timeout: 1500,
      windowsHide: true,
      maxBuffer: 64 * 1024 * 1024,
    });
    return stdout.length > 0 ? stdout : undefined;
  } catch {
    return undefined;
  }
}

function sanitizeInputFileName(value: string): string {
  const leaf = basename(String(value).replace(/\\/gu, "/"));
  const cleaned = leaf.replace(/[<>:"/\\|?*\x00-\x1f\s]+/gu, "-").replace(/^-+|-+$/gu, "");
  return cleaned || "clipboard.png";
}

function relativeWorkspacePath(cwd: string, absolute: string): string {
  return absolute.startsWith(resolve(cwd))
    ? absolute.slice(resolve(cwd).length + 1).replaceAll(sep, "/")
    : absolute.replaceAll(sep, "/");
}
