import type { Session, TaskManagement } from "@tura/gateway-sdk";
import { describe, expect, test } from "bun:test";
import {
  firstRunnableTask,
  planSessionStatus,
  sortedSessionTasks,
  taskStartCondition,
  timedTaskPatch,
} from "./tasks";

function session(status: Session["status"], task_management?: TaskManagement): Session {
  return {
    id: `session-${status}-${task_management?.status ?? "none"}`,
    status,
    task_management,
  };
}

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
    expect(timedTaskPatch("scheduled_task", "2026-05-27T10:00", undefined)).toEqual({
      start_condition: "scheduled_task",
      start_at: "2026-05-27T10:00",
      poll_interval: { m: 0, d: 0, h: 0, s: 0 },
    });
    expect(timedTaskPatch("polling_task", "2026-05-27T10:00", { h: 2 })).toEqual({
      start_condition: "polling_task",
      start_at: "2026-05-27T10:00",
      poll_interval: { m: 0, d: 0, h: 2, s: 0 },
    });
  });

  test("uses runtime busy as the only running board state", () => {
    expect(
      planSessionStatus(
        session("idle", {
          status: "doing",
          task_summary: "Stale doing task",
        }),
      ),
    ).toBe("todo");

    expect(
      planSessionStatus(
        session("busy", {
          status: "question",
          task_summary: "Waiting question",
        }),
      ),
    ).toBe("doing");
  });

  test("shows idle question tasks as feedback and otherwise falls back to todo", () => {
    expect(
      planSessionStatus(
        session("idle", {
          status: "question",
          task_summary: "Need input",
        }),
      ),
    ).toBe("question");

    expect(
      planSessionStatus(
        session("idle", {
          status: "waiting_user",
          task_summary: "No task lane",
        }),
      ),
    ).toBe("todo");
  });

  test("does not treat question or completed tasks as runnable", () => {
    expect(
      firstRunnableTask(
        session("idle", {
          tasks: [
            { task_id: "q", status: "question", task_summary: "Need input" },
            { task_id: "d", status: "done", task_summary: "Already done" },
          ],
        }),
      ),
    ).toBeUndefined();
  });

  test("hides completed and archived tasks from frontend task lists", () => {
    const visible = sortedSessionTasks(
      session("idle", {
        tasks: [
          { task_id: "todo", status: "todo", task_summary: "Visible task" },
          { task_id: "done", status: "done", task_summary: "Done task" },
          {
            task_id: "archived",
            status: "archived",
            task_summary: "Archived task",
          },
        ],
      }),
    );

    expect(visible.map((task) => task.task_id)).toEqual(["todo"]);
  });
});
