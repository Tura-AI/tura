export type HealthResponse = {
  healthy: boolean;
  version: string;
  root?: string;
  home?: string;
  exe_dir?: string;
  pid?: number;
  process_start_time?: number;
  dev_log_path?: string;
};
