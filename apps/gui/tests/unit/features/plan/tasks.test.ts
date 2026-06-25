import type { Session, TaskManagement } from "@tura/gateway-sdk";
import { describe, expect, test } from "bun:test";
import {
  appendTaskToSession,
  applyTaskPatchToSession,
  firstRunnableTask,
  materializeComposerContent,
  planSessionStatus,
  queuedSessionTasks,
  reorderTasksInSession,
  shouldShowSessionAttention,
  sortedSessionTasks,
  taskStartCondition,
  timedSessionTasks,
  timedTaskDisplayDate,
  timedTaskPatch,
} from "../../../../app/src/features/plan/tasks";

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

  test("keeps running session attention visible after acknowledgement", () => {
    expect(shouldShowSessionAttention(session("busy"), false)).toBe(true);
    expect(shouldShowSessionAttention(session("busy"), true)).toBe(true);
  });

  test("does not keep completed non-planning sessions busy from a stale doing task", () => {
    const staleDoing = session("idle", {
      status: "doing",
      task_summary: "Already summarized",
    });

    expect(planSessionStatus(staleDoing)).toBe("todo");
    expect(shouldShowSessionAttention(staleDoing, false)).toBe(false);
  });

  test("allows question and done attention to be acknowledged", () => {
    const question = session("idle", { status: "question", task_summary: "Need input" });
    const done = session("idle", { status: "done", task_summary: "Complete" });

    expect(shouldShowSessionAttention(question, false)).toBe(true);
    expect(shouldShowSessionAttention(question, true)).toBe(false);
    expect(shouldShowSessionAttention(done, false)).toBe(true);
    expect(shouldShowSessionAttention(done, true)).toBe(false);
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

  test("patches, appends, and reorders task lists without losing sibling tasks", () => {
    const initial = session("idle", {
      tasks: [
        { task_id: "a", step: 1, status: "todo", task_summary: "A" },
        { task_id: "b", step: 2, status: "todo", task_summary: "B" },
      ],
    });

    const patched = applyTaskPatchToSession(initial, {
      task_id: "b",
      task_summary: "B updated",
      deliverable: "Ship it",
    });
    expect(patched.task_management?.tasks?.map((task) => task.task_summary)).toEqual([
      "A",
      "B updated",
    ]);

    const appended = appendTaskToSession(patched, { task_summary: "C" });
    expect(appended.task_management?.tasks?.map((task) => task.task_id)).toEqual([
      "a",
      "b",
      "session-idle-none:2",
    ]);

    const reordered = reorderTasksInSession(appended, [
      appended.task_management!.tasks![2]!,
      appended.task_management!.tasks![0]!,
      appended.task_management!.tasks![1]!,
    ]);
    expect(reordered.task_management?.tasks?.map((task) => [task.task_summary, task.step])).toEqual(
      [
        ["C", 1],
        ["A", 2],
        ["B updated", 3],
      ],
    );
  });

  test("pipeline queue includes scheduled and polling tasks ordered by step", () => {
    const tasks = queuedSessionTasks(
      session("idle", {
        tasks: [
          {
            task_id: "scheduled",
            step: 1,
            status: "todo",
            task_summary: "Scheduled",
            start_condition: "scheduled_task",
            start_at: "2026-06-08T10:00:00Z",
          },
          { task_id: "queued-b", step: 3, status: "todo", task_summary: "Queued B" },
          {
            task_id: "polling",
            step: 2,
            status: "todo",
            task_summary: "Polling",
            start_condition: "polling_task",
            start_at: "2026-06-08T10:00:00Z",
            poll_interval: { h: 1 },
          },
          {
            task_id: "queued-a",
            step: 2,
            status: "todo",
            task_summary: "Queued A",
            start_condition: "session_idle",
          },
        ],
      }),
    );

    expect(tasks.map((task) => task.task_id)).toEqual([
      "scheduled",
      "polling",
      "queued-a",
      "queued-b",
    ]);
  });

  test("calendar treats gateway timed tasks without status as todo", () => {
    const tasks = timedSessionTasks(
      session("idle", {
        tasks: [
          {
            task_id: "scheduled",
            task_summary: "Scheduled",
            start_condition: "scheduled_task",
            start_at: "2026-06-08T10:00:00Z",
          },
          {
            task_id: "polling",
            task_summary: "Polling",
            start_condition: "polling_task",
            start_at: "2026-06-08T11:00:00Z",
            poll_interval: { h: 1 },
          },
        ],
      }),
    );

    expect(tasks.map((task) => task.task_id)).toEqual(["scheduled", "polling"]);
  });

  test("polling task display date advances from start_at by interval", () => {
    const date = timedTaskDisplayDate(
      {
        status: "todo",
        task_summary: "Poll",
        start_condition: "polling_task",
        start_at: "2026-06-08T10:00:00Z",
        poll_interval: { h: 2 },
      },
      new Date("2026-06-08T15:10:00Z").getTime(),
    );

    expect(date?.toISOString()).toBe("2026-06-08T16:00:00.000Z");
  });

  test("timed session tasks keeps multiple scheduled tasks for calendar rendering", () => {
    const tasks = timedSessionTasks(
      session("idle", {
        tasks: [
          {
            task_id: "first",
            step: 1,
            status: "todo",
            task_summary: "First",
            start_condition: "scheduled_task",
            start_at: "2026-06-08T10:00:00Z",
          },
          {
            task_id: "second",
            step: 2,
            status: "todo",
            task_summary: "Second",
            start_condition: "scheduled_task",
            start_at: "2026-06-09T10:00:00Z",
          },
        ],
      }),
    );

    expect(tasks.map((task) => task.task_id)).toEqual(["first", "second"]);
  });

  test("materializes composer image and file attachments in prompt order", () => {
    const content = materializeComposerContent("Investigate\n[[image:img1]]\n[[file:file1]]", [
      {
        id: "img1",
        name: "screen.png",
        dataUrl: "data:image/png;base64,abc",
        kind: "image",
      },
      {
        id: "file1",
        name: "notes.txt",
        dataUrl: "blob:notes",
        kind: "file",
      },
      {
        id: "img2",
        name: "extra.png",
        dataUrl: "data:image/png;base64,def",
        kind: "image",
      },
    ]);

    expect(content).toContain("[Image 1: screen.png]");
    expect(content).toContain("[MEDIA:data:image/png;base64,abc:MEDIA]");
    expect(content).toContain("[File 2: notes.txt]");
    expect(content).toContain("[Image 3: extra.png]");
    expect(content).not.toContain("[[image:img1]]");
  });
});
