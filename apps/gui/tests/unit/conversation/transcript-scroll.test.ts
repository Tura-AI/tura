import { describe, expect, test } from "bun:test";
import {
  TRANSCRIPT_FOLLOW_BOTTOM_RATIO,
  transcriptBottomDistance,
  transcriptFollowBottomThreshold,
  transcriptNearBottom,
} from "../../../app/src/conversation/transcript-scroll";

function scrollMetrics(scrollableHeight: number, bottomDistance: number) {
  const clientHeight = 1000;
  return {
    clientHeight,
    scrollHeight: clientHeight + scrollableHeight,
    scrollTop: scrollableHeight - bottomDistance,
  };
}

describe("transcript live follow-bottom detection", () => {
  test("uses a 0.5% near-bottom recognition band", () => {
    const element = scrollMetrics(20_000, 100);

    expect(TRANSCRIPT_FOLLOW_BOTTOM_RATIO).toBe(0.005);
    expect(transcriptFollowBottomThreshold(element)).toBe(100);
    expect(transcriptBottomDistance(element)).toBe(100);
    expect(transcriptNearBottom(element)).toBe(true);
  });

  test("does not auto-follow when the transcript is outside the 0.5% band", () => {
    expect(transcriptNearBottom(scrollMetrics(20_000, 101))).toBe(false);
  });

  test("keeps only a tiny pixel tolerance for browser scroll rounding", () => {
    expect(transcriptNearBottom(scrollMetrics(100, 2))).toBe(true);
    expect(transcriptNearBottom(scrollMetrics(100, 3))).toBe(false);
  });
});
