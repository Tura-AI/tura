export type PromptPart = {
  id?: string;
  type: "text";
  text: string;
};

export type PromptAsyncRequest = {
  parts: PromptPart[];
  messageID?: string;
  model?: string | { providerID: string; modelID: string };
  agent?: string;
  variant?: string;
  model_acceleration_enabled?: boolean;
  system?: string;
};

