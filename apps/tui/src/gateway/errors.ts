export class GatewayHttpError extends Error {
  constructor(
    public status: number,
    public url: string,
    message: string,
    public body?: string,
  ) {
    super(message);
  }
}

export function errorMessage(error: unknown): string {
  if (error instanceof GatewayHttpError) {
    return `${error.message}${error.body ? `: ${error.body}` : ""}`;
  }
  return error instanceof Error ? error.message : String(error);
}
