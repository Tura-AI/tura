export const TRANSCRIPT_FOLLOW_BOTTOM_RATIO = 0.005;
export const TRANSCRIPT_FOLLOW_BOTTOM_MIN_PX = 2;

type TranscriptScrollMetrics = Pick<HTMLElement, "clientHeight" | "scrollHeight" | "scrollTop">;

export function transcriptBottomDistance(element: TranscriptScrollMetrics) {
  return Math.max(0, element.scrollHeight - element.scrollTop - element.clientHeight);
}

export function transcriptFollowBottomThreshold(element: TranscriptScrollMetrics) {
  const scrollableHeight = Math.max(0, element.scrollHeight - element.clientHeight);
  return Math.max(
    TRANSCRIPT_FOLLOW_BOTTOM_MIN_PX,
    scrollableHeight * TRANSCRIPT_FOLLOW_BOTTOM_RATIO,
  );
}

export function transcriptNearBottom(element: TranscriptScrollMetrics) {
  return transcriptBottomDistance(element) <= transcriptFollowBottomThreshold(element);
}
