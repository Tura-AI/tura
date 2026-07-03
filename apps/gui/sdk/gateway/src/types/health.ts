export type HealthResponse = {
  healthy: boolean;
  version: string;
  root?: string;
  home?: string;
  exe_dir?: string;
  dev_log_path?: string;
};
