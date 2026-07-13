export interface AboutInfo {
  release_version: string;
  system: {
    operating_system: string;
    os_version: string;
    architecture: string;
  };
}

export type AboutOpenTarget = "report_bug" | "contribute" | "contact";

export interface AboutStarResponse {
  outcome: "starred" | "opened";
}

export interface AboutOpenResponse {
  opened: boolean;
  target: AboutOpenTarget;
}

export interface AboutUpdate {
  current_version: string;
  latest_version: string;
}

export interface AboutUpdateCheckResponse {
  update?: AboutUpdate;
}

export interface AboutUpdateInstallResponse {
  scheduled: boolean;
  version: string;
}
