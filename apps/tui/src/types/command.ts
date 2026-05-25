export interface TuraCommand {
  name: string;
  description: string;
  agent?: string | null;
  model?: string | null;
  source?: string;
  template?: string | null;
  subtask?: boolean;
  hints?: string[];
}
