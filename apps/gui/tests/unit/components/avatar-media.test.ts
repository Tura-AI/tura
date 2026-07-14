import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import type { PersonaMediaConfig } from "@tura/gateway-sdk";
import { describe, expect, test } from "bun:test";
import {
  avatarImageKeyForLoaded,
  avatarPixelAfterThreshold,
  type AvatarExpressionInfo,
} from "../../../app/src/components/avatar/agent-avatar-rendering";

const repoRoot = resolve(process.cwd(), "..", "..");
const personaRoot = join(repoRoot, "personas", "src");
const guiPersonaAssets = join(repoRoot, "apps", "gui", "app", "public", "assets", "persona");

function personaAsset(path: string): string {
  const normalized = path.replace(/\\/gu, "/");
  const match = normalized.match(/(?:^|\/)personas\/src\/([^/]+)\/media\/(.+)$/u);
  if (!match) {
    throw new Error(`unsupported persona asset path: ${path}`);
  }
  return join(personaRoot, match[1]!, "media", ...match[2]!.split("/"));
}

async function mediaConfig(role: string): Promise<PersonaMediaConfig> {
  const raw = await readFile(
    join(repoRoot, "personas", "src", role, "persona_config.json"),
    "utf8",
  );
  const config = JSON.parse(raw) as { media: PersonaMediaConfig };
  return config.media;
}

describe("agent avatar media", () => {
  test("keeps one 200px JPEG source for every configured persona image", async () => {
    const missing: string[] = [];
    for (const role of ["tura", "wonderful", "pidan"]) {
      const media = await mediaConfig(role);
      for (const expression of media.expressions ?? []) {
        if (
          !expression.grid_path.endsWith(".jpg") ||
          !existsSync(personaAsset(expression.grid_path))
        ) {
          missing.push(expression.grid_path);
        }
        for (const frame of Object.values(expression.frames)) {
          if (!frame.endsWith(".jpg") || !existsSync(personaAsset(frame))) {
            missing.push(frame);
          }
        }
      }
    }

    expect(missing).toEqual([]);
    expect(existsSync(guiPersonaAssets)).toBe(false);
  });

  test("uses a loaded default expression frame before the global fallback", () => {
    const expressions: AvatarExpressionInfo[] = [
      { id: "laugh", aliases: [], frames: { right: "laugh/right.jpg" } },
      { id: "vigilant", aliases: [], frames: { right: "vigilant/right.jpg" } },
    ];

    expect(
      avatarImageKeyForLoaded(
        expressions,
        ["vigilant:right"],
        "laugh",
        "right",
        "right",
        "vigilant",
      ),
    ).toBe("vigilant:right");
  });

  test("keeps light theme foreground pixels black and background transparent", () => {
    expect(avatarPixelAfterThreshold(24, 255, 160, false)).toEqual({ value: 0, alpha: 255 });
    expect(avatarPixelAfterThreshold(240, 255, 160, false)).toEqual({ value: 255, alpha: 0 });
  });

  test("uses white negative space instead of inverting foreground in dark theme", () => {
    expect(avatarPixelAfterThreshold(24, 255, 160, true)).toEqual({ value: 255, alpha: 0 });
    expect(avatarPixelAfterThreshold(240, 0, 160, true)).toEqual({ value: 255, alpha: 255 });
  });
});
