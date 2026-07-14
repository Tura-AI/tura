export const EmptyBorder = {
  topLeft: "",
  bottomLeft: "",
  vertical: "",
  topRight: "",
  bottomRight: "",
  horizontal: " ",
  bottomT: "",
  topT: "",
  cross: "",
  leftT: "",
  rightT: "",
};

export const SplitBorder = {
  border: ["left" as const, "right" as const],
  customBorderChars: {
    ...EmptyBorder,
    vertical: "▏",
  },
};

export const SplitBorderFallback = {
  ...SplitBorder,
  customBorderChars: {
    ...SplitBorder.customBorderChars,
    vertical: "|",
  },
};
