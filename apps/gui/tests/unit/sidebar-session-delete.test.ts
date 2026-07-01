import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const sidebarSource = readFileSync(
  resolve(import.meta.dir, "../../app/src/components/sidebar.tsx"),
  "utf8",
);
const workspaceChildrenSource = readFileSync(
  resolve(import.meta.dir, "../../app/src/components/sidebar/workspace-children.tsx"),
  "utf8",
);

describe("sidebar session deletion contract", () => {
  test("session row action asks for confirmation before deleting", () => {
    expect(workspaceChildrenSource).toContain('title={t("delete")}');
    expect(workspaceChildrenSource).toContain("props.onDelete(session())");
    expect(workspaceChildrenSource).not.toContain('action="archive"');
    expect(workspaceChildrenSource).not.toContain("onArchive");

    expect(sidebarSource).toContain("createSignal<Session>()");
    expect(sidebarSource).toContain("onDeleteSession={setConfirmDeleteSession}");
    expect(sidebarSource).toContain("setConfirmDeleteSession(session)");
    expect(sidebarSource).toContain("ConfirmSessionDeleteDialog");
    expect(sidebarSource).toContain("props.onDeleteSession(session.id)");
  });
});
