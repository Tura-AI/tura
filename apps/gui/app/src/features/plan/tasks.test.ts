import { describe, expect, test } from "bun:test";
import type { TaskManagement } from "@tura/gateway-sdk";
import { taskStartCondition, timedTaskPatch } from "./tasks";

describe("plan task contract", () => {
  test("prefers explicit start_condition over lifecycle status", () => {
    const task: TaskManagement = {
      status: "todo",
      start_condition: "session_idle",
      task_summary: "Run when idle",
    };

    expect(taskStartCondition(task)).toBe("session_idle");
  });

  test("keeps legacy start-condition encoded in status readable", () => {
    const task = {
      status: "session_idle",
      task_summary: "Legacy idle task",
    } as unknown as TaskManagement;

    expect(taskStartCondition(task)).toBe("session_idle");
  });

  test("treats untimed task-list entries as queued by default", () => {
    const task: TaskManagement = {
      status: "todo",
      task_summary: "Queued task",
    };

    expect(taskStartCondition(task)).toBe("session_idle");
  });

  test("emits explicit start_condition for queued and timed tasks", () => {
    expect(timedTaskPatch("session_idle", undefined, undefined)).toEqual({
      start_condition: "session_idle",
    });
    expect(timedTaskPatch("scheduled_task", "2026-05-27T10:00", undefined))
      .toEqual({
        start_condition: "scheduled_task",
        start_at: "2026-05-27T10:00",
        poll_interval: { m: 0, d: 0, h: 0, s: 0 },
      });
    expect(timedTaskPatch("polling_task", "2026-05-27T10:00", { h: 2 }))
      .toEqual({
        start_condition: "polling_task",
        start_at: "2026-05-27T10:00",
        poll_interval: { m: 0, d: 0, h: 2, s: 0 },
      });
  });
});
