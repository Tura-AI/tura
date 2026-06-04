export interface SessionConfig {
  language?: string | null;
  model?: string | null;
  active_provider?: string | null;
  active_model?: string | null;
  active_agent?: string | null;
  session_type?: string | null;
  model_variant?: string | null;
  model_acceleration_enabled?: boolean;
  context_message_limit?: number;
  kill_processes_on_start?: boolean;
  validator_enabled?: boolean;
  command_run_stall_guard_profile?: string | null;
  command_run_stall_guard_check_secs?: number;
  command_run_stall_guard_identical_checks?: number;
  [key: string]: unknown;
}
