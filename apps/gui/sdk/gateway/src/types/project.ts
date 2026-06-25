export type Project = {
  id: string;
  worktree: string;
  vcs?: string | null;
  name?: string | null;
  icon?: {
    url?: string | null;
    override_?: string | null;
    color?: string | null;
  } | null;
  time?: {
    created?: number;
    updated?: number;
    initialized?: number | null;
  };
};

export type CurrentProjectResponse = {
  project?: Project | null;
};

