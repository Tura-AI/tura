export type VcsInfo = {
  branch: string;
  default_branch: string;
};

export type VcsDiffResponse = {
  files: FileDiff[];
};

export type FileDiff = {
  old_file_name: string;
  new_file_name: string;
  hunks: DiffHunk[];
};

export type DiffHunk = {
  old_start: number;
  old_lines: number;
  new_start: number;
  new_lines: number;
  lines: string[];
};

export type FileInfo = {
  name: string;
  path: string;
  type: "directory" | "file" | string;
  absolute: string;
  ignored: boolean;
  git_status?: string | null;
  size_bytes?: number | null;
  modified_at?: number | null;
};

export type FileContentResponse = {
  type: "text" | "binary" | "media" | string;
  content: string;
  encoding?: string | null;
  mimeType?: string | null;
};

export type FileOpenResponse = {
  path: string;
  opened: boolean;
};

export type FileInputSaveRequest = {
  name: string;
  content: string;
  encoding: "base64";
  mimeType?: string | null;
};

export type FileInputSaveResponse = {
  path: string;
  absolute: string;
  name: string;
  mimeType?: string | null;
  size_bytes: number;
};
