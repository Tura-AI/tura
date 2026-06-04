export type QueryKey = readonly [
  scope: string,
  ...parts: Array<string | number | boolean | undefined>,
];

export const queryKeys = {
  health: (): QueryKey => ["global", "health"],
  config: (): QueryKey => ["global", "config"],
  me: (): QueryKey => ["global", "me"],
  workspaces: (): QueryKey => ["global", "workspaces"],
  providers: (): QueryKey => ["execution", "providers"],
  agents: (directory?: string): QueryKey => ["execution", "agents", directory],
  sessions: (directory?: string): QueryKey => [
    "execution",
    "sessions",
    directory,
  ],
  messages: (sessionId: string): QueryKey => [
    "execution",
    "messages",
    sessionId,
  ],
  files: (directory?: string, path = ""): QueryKey => [
    "execution",
    "files",
    directory,
    path,
  ],
  issues: (workspaceId?: string, search?: string): QueryKey => [
    "workspace",
    "issues",
    workspaceId,
    search,
  ],
  projects: (workspaceId?: string): QueryKey => [
    "workspace",
    "projects",
    workspaceId,
  ],
} as const;

export function serializeQueryKey(key: QueryKey): string {
  return key.map((part) => String(part ?? "")).join("\u001f");
}

export function createQueryCache() {
  const cache = new Map<string, unknown>();
  return {
    get<T>(key: QueryKey): T | undefined {
      return cache.get(serializeQueryKey(key)) as T | undefined;
    },
    set<T>(key: QueryKey, value: T): T {
      cache.set(serializeQueryKey(key), value);
      return value;
    },
    invalidate(prefix: QueryKey): void {
      const serializedPrefix = serializeQueryKey(prefix);
      for (const key of cache.keys()) {
        if (
          key === serializedPrefix ||
          key.startsWith(`${serializedPrefix}\u001f`)
        ) {
          cache.delete(key);
        }
      }
    },
    clear(): void {
      cache.clear();
    },
  };
}
