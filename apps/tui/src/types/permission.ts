export interface PermissionRequest {
  id: string;
  sessionID?: string;
  permission: string;
  args?: Record<string, unknown>;
}

export interface QuestionRequest {
  id: string;
  sessionID?: string;
  question: string;
  metadata?: Record<string, unknown>;
}
