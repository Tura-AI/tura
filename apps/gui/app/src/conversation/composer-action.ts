export type ComposerActionState = {
  kind: "send" | "stop";
  disabled: boolean;
};

export function composerActionState(input: {
  text: string;
  imageCount: number;
  running: boolean;
  submitting: boolean;
  submitDisabled?: boolean;
  hasStopHandler: boolean;
}): ComposerActionState {
  const draftEmpty = !input.text.trim() && input.imageCount === 0;
  const kind = input.running && draftEmpty && input.hasStopHandler ? "stop" : "send";
  return {
    kind,
    disabled:
      input.submitting || (kind === "send" && (Boolean(input.submitDisabled) || draftEmpty)),
  };
}
