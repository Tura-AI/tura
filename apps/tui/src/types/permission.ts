export interface PermissionRequest {
  id: string;
  session_id?: string;
  sessionID?: string;
  permission: string;
  args?: Record<string, unknown>;
}

export interface PermissionReplyResponse {
  success: boolean;
}

export interface QuestionRequest {
  id: string;
  session_id?: string;
  sessionID?: string;
  question: string;
  metadata?: Record<string, unknown>;
}
