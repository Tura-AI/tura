import type { Session } from "@tura/gateway-sdk";
import { sessionUpdatedAt } from "./global-store";
import { normalizeTimeMs } from "../utils/app-format";

export type SessionTreeRow = {
  session: Session;
  depth: number;
};

export function sessionParentId(session: Session): string | undefined {
  const parentId = session.parent_id?.trim();
  return parentId || undefined;
}

export function isRootSession(session: Session): boolean {
  return !sessionParentId(session);
}

export function rootSessions(sessions: Session[]): Session[] {
  return sortSessions(sessions.filter(isRootSession));
}

export function topmostSessionId(
  sessions: Session[],
  sessionId: string | undefined,
): string | undefined {
  if (!sessionId) {
    return undefined;
  }
  const byId = new Map(sessions.map((session) => [session.id, session]));
  let current = byId.get(sessionId);
  if (!current) {
    return undefined;
  }
  const visited = new Set<string>();
  while (current) {
    if (visited.has(current.id)) {
      return current.id;
    }
    visited.add(current.id);
    const parentId = sessionParentId(current);
    if (!parentId) {
      return current.id;
    }
    const parent = byId.get(parentId);
    if (!parent) {
      return current.id;
    }
    current = parent;
  }
  return undefined;
}

export function visibleSessionTreeRows(
  sessions: Session[],
  selectedSessionId: string | undefined,
  options: { expandedRoots?: boolean; collapsedRootLimit?: number } = {},
): SessionTreeRow[] {
  const expandedRootId = topmostSessionId(sessions, selectedSessionId);
  const rootLimit = options.collapsedRootLimit ?? 5;
  const roots = rootSessions(sessions);
  const visibleRoots = options.expandedRoots
    ? roots
    : roots
        .slice(0, rootLimit)
        .concat(
          expandedRootId &&
            !roots
              .slice(0, rootLimit)
              .some((root) => root.id === expandedRootId)
            ? roots.filter((root) => root.id === expandedRootId)
            : [],
        );
  return visibleRoots.flatMap((root) =>
    root.id === expandedRootId
      ? expandedSessionRows(sessions, root.id)
      : [{ session: root, depth: 0 }],
  );
}

export function hiddenRootSessionCount(
  sessions: Session[],
  selectedSessionId: string | undefined,
  rootLimit = 5,
): number {
  const expandedRootId = topmostSessionId(sessions, selectedSessionId);
  const roots = rootSessions(sessions);
  return Math.max(
    0,
    roots.filter(
      (root, index) => index >= rootLimit && root.id !== expandedRootId,
    ).length,
  );
}

function expandedSessionRows(
  sessions: Session[],
  rootId: string,
): SessionTreeRow[] {
  const childrenByParent = new Map<string, Session[]>();
  for (const session of sessions) {
    const parentId = sessionParentId(session);
    if (!parentId) {
      continue;
    }
    const children = childrenByParent.get(parentId) ?? [];
    children.push(session);
    childrenByParent.set(parentId, children);
  }
  for (const [parentId, children] of childrenByParent) {
    childrenByParent.set(parentId, sortSessions(children));
  }
  const byId = new Map(sessions.map((session) => [session.id, session]));
  const root = byId.get(rootId);
  if (!root) {
    return [];
  }
  const rows: SessionTreeRow[] = [];
  const append = (session: Session, depth: number, ancestors: Set<string>) => {
    rows.push({ session, depth });
    if (ancestors.has(session.id)) {
      return;
    }
    const nextAncestors = new Set(ancestors);
    nextAncestors.add(session.id);
    for (const child of childrenByParent.get(session.id) ?? []) {
      append(child, depth + 1, nextAncestors);
    }
  };
  append(root, 0, new Set());
  return rows;
}

function sortSessions(sessions: Session[]): Session[] {
  return [...sessions].sort(
    (left, right) =>
      normalizeTimeMs(sessionUpdatedAt(right) ?? 0) -
      normalizeTimeMs(sessionUpdatedAt(left) ?? 0),
  );
}
