import type { GatewayEventEnvelope } from "./types";

export type GatewayEventHandler = (event: GatewayEventEnvelope) => void;
export type GatewayEventErrorHandler = (error: Event | Error) => void;

export type GatewayEventStream = {
  close: () => void;
};

export function connectGatewayEvents(input: {
  baseUrl: string;
  onEvent: GatewayEventHandler;
  onError?: GatewayEventErrorHandler;
}): GatewayEventStream {
  if (typeof EventSource === "undefined") {
    input.onError?.(new Error("EventSource is not available"));
    return { close: () => undefined };
  }

  const url = new URL("/event", input.baseUrl);
  const eventSource = new EventSource(url.toString());

  eventSource.onmessage = (message) => {
    try {
      const parsed = JSON.parse(message.data) as GatewayEventEnvelope;
      if (parsed && typeof parsed === "object" && "payload" in parsed) {
        input.onEvent(parsed);
      }
    } catch (error) {
      input.onError?.(error instanceof Error ? error : new Error(String(error)));
    }
  };

  eventSource.onerror = (event) => {
    input.onError?.(event);
  };

  return {
    close: () => eventSource.close(),
  };
}
