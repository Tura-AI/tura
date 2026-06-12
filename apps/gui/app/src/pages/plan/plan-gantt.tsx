import { type Session, type TaskManagement } from "@tura/gateway-sdk";
import { For, Show, createMemo, createSignal } from "solid-js";
import { t } from "../../i18n";
import { classNames } from "../../state/format";
import { sessionTitle } from "../../state/global-store";

import {
  planQueuedSessions,
  planSessionStatus,
  planTaskTitle,
  queuedSessionTasks,
  shortSessionId,
  sortedSessionTasks,
  taskNonceId,
  taskPlanStatus,
  taskSummaryText,
} from "../../features/plan/tasks";

type PipelineDropTarget = {
  sessionId: string;
  index: number;
};

type PipelinePointerDrag = {
  session: Session;
  task: TaskManagement;
  startX: number;
  startY: number;
  moved: boolean;
};

export function PlanGanttView(props: {
  sessions: Session[];
  selectedSessionId?: string;
  selectedTaskNonceId?: string;
  onOpenSession: (session: Session) => void;
  onEditTask: (session: Session, task: TaskManagement) => void;
  onReorder: (session: Session, tasks: TaskManagement[]) => void;
}) {
  const [dropTarget, setDropTarget] = createSignal<PipelineDropTarget>();
  let activeDragTask: { sessionId: string; taskId: string } | undefined;
  let activePointerDrag: PipelinePointerDrag | undefined;
  let suppressTaskClick: string | undefined;
  const boundTrackElements = new WeakSet<HTMLElement>();
  const boundTaskElements = new WeakSet<HTMLElement>();
  const queuedRows = createMemo(() =>
    planQueuedSessions(props.sessions).map((session) => ({
      session,
      tasks: queuedSessionTasks(session),
    })),
  );
  const maxSteps = createMemo(() =>
    Math.max(
      1,
      ...queuedRows().map((row) => Math.max(...row.tasks.map(taskStep), row.tasks.length)),
    ),
  );

  function taskStep(task: TaskManagement): number {
    return typeof task.step === "number" && task.step > 0 ? task.step : 1;
  }

  function pipelineDropFromPoint(
    point: { x: number; y: number },
    sourceSessionId: string,
  ): PipelineDropTarget | undefined {
    const element = document.elementFromPoint(point.x, point.y) as HTMLElement | undefined;
    const row =
      element?.closest<HTMLElement>(".plan-pipeline-row") ??
      Array.from(document.querySelectorAll<HTMLElement>(".plan-pipeline-row")).find((item) => {
        const rect = item.getBoundingClientRect();
        return point.y >= rect.top && point.y <= rect.bottom;
      });
    const sessionId = row?.dataset.sessionId;
    if (!row || !sessionId || sessionId !== sourceSessionId) {
      return undefined;
    }
    const track = row.querySelector<HTMLElement>(".plan-pipeline-track");
    if (!track) {
      return undefined;
    }
    return {
      sessionId,
      index: pipelineStepFromPoint(track, point.x),
    };
  }

  function pipelineStepFromPoint(track: HTMLElement, x: number): number {
    const rect = track.getBoundingClientRect();
    const width = rect.width / maxSteps();
    if (width <= 0) {
      return 0;
    }
    return Math.max(0, Math.min(maxSteps() - 1, Math.floor((x - rect.left) / width)));
  }

  function reorderTaskTo(
    session: Session,
    sourceTask: TaskManagement,
    target: PipelineDropTarget,
  ): boolean {
    if (target.sessionId !== session.id) {
      return false;
    }
    const sourceNonce = taskNonceId(sourceTask);
    if (!sourceNonce) {
      return false;
    }
    const current = queuedSessionTasks(session);
    const sourceIndex = current.findIndex((task) => taskNonceId(task) === sourceNonce);
    if (sourceIndex < 0) {
      return false;
    }
    const next = current.filter((task) => taskNonceId(task) !== sourceNonce);
    const source = current[sourceIndex];
    if (!source) {
      return false;
    }
    const insertIndex = Math.max(0, Math.min(target.index, next.length));
    next.splice(insertIndex, 0, source);
    const visibleTaskIds = new Set(next.map(taskNonceId).filter(Boolean));
    const hiddenTasks = sortedSessionTasks(session).filter(
      (task) => !visibleTaskIds.has(taskNonceId(task)),
    );
    props.onReorder(session, [...next, ...hiddenTasks]);
    return true;
  }

  function beginTaskDrag(event: DragEvent, session: Session, task: TaskManagement) {
    const taskId = taskNonceId(task) ?? "";
    activeDragTask = taskId ? { sessionId: session.id, taskId } : undefined;
    event.dataTransfer?.setData("text/session-id", session.id);
    event.dataTransfer?.setData("text/task-id", taskId);
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = "move";
    }
  }

  function updateDropTarget(event: DragEvent, session: Session) {
    event.preventDefault();
    setDropTarget(pipelineDropFromPoint({ x: event.clientX, y: event.clientY }, session.id));
  }

  function dropTask(event: DragEvent, session: Session) {
    event.preventDefault();
    const sourceSessionId =
      event.dataTransfer?.getData("text/session-id") || activeDragTask?.sessionId;
    const sourceTaskId = event.dataTransfer?.getData("text/task-id") || activeDragTask?.taskId;
    if (!sourceTaskId || sourceSessionId !== session.id) {
      setDropTarget(undefined);
      activeDragTask = undefined;
      return;
    }
    const sourceTask = queuedSessionTasks(session).find(
      (task) => taskNonceId(task) === sourceTaskId,
    );
    const target = pipelineDropFromPoint({ x: event.clientX, y: event.clientY }, session.id);
    setDropTarget(undefined);
    activeDragTask = undefined;
    if (sourceTask && target) {
      reorderTaskTo(session, sourceTask, target);
    }
  }

  function dropClass(session: Session, task: TaskManagement): string | undefined {
    const target = dropTarget();
    if (target?.sessionId !== session.id || target.index + 1 !== taskStep(task)) {
      return undefined;
    }
    return "drop-step";
  }

  function bindTrackElement(element: HTMLElement, session: Session) {
    if (boundTrackElements.has(element)) {
      return;
    }
    boundTrackElements.add(element);
    element.addEventListener("dragover", (event) => updateDropTarget(event, session));
    element.addEventListener("drop", (event) => dropTask(event, session));
    element.addEventListener("dragleave", () => setDropTarget(undefined));
  }

  function bindTaskElement(element: HTMLElement, session: Session, task: TaskManagement) {
    if (boundTaskElements.has(element)) {
      return;
    }
    boundTaskElements.add(element);
    element.addEventListener("dragstart", (event) => beginTaskDrag(event, session, task));
    element.addEventListener("dragend", () => {
      setDropTarget(undefined);
      activeDragTask = undefined;
    });
  }

  function beginPointerDrag(event: PointerEvent, session: Session, task: TaskManagement) {
    if (event.button !== 0) {
      return;
    }
    activePointerDrag = {
      session,
      task,
      startX: event.clientX,
      startY: event.clientY,
      moved: false,
    };
    elementFromEvent(event)?.setPointerCapture?.(event.pointerId);
  }

  function updatePointerDrag(event: PointerEvent) {
    const drag = activePointerDrag;
    if (!drag) {
      return;
    }
    const deltaX = Math.abs(event.clientX - drag.startX);
    const deltaY = Math.abs(event.clientY - drag.startY);
    if (!drag.moved && deltaX < 4 && deltaY < 4) {
      return;
    }
    drag.moved = true;
    setDropTarget(pipelineDropFromPoint({ x: event.clientX, y: event.clientY }, drag.session.id));
  }

  function finishPointerDrag(event: PointerEvent) {
    const drag = activePointerDrag;
    activePointerDrag = undefined;
    if (!drag?.moved) {
      setDropTarget(undefined);
      return;
    }
    const target = pipelineDropFromPoint({ x: event.clientX, y: event.clientY }, drag.session.id);
    setDropTarget(undefined);
    if (target && reorderTaskTo(drag.session, drag.task, target)) {
      suppressTaskClick = taskNonceId(drag.task);
      window.setTimeout(() => {
        suppressTaskClick = undefined;
      }, 0);
    }
  }

  function elementFromEvent(event: PointerEvent): HTMLElement | undefined {
    return event.currentTarget instanceof HTMLElement ? event.currentTarget : undefined;
  }

  return (
    <section class="plan-gantt plan-pipeline" aria-label={t("gantt")}>
      <div
        class="plan-timeline-grid plan-pipeline-grid"
        style={{ "--plan-days": String(maxSteps()) }}
      >
        <div class="plan-timeline-scale plan-pipeline-scale">
          <header class="plan-calendar-title plan-timeline-title plan-pipeline-title">
            <strong>{t("gantt")}</strong>
          </header>
          <span class="plan-timeline-left-head" aria-hidden="true"></span>
          <For each={Array.from({ length: maxSteps() })}>
            {(_, index) => (
              <span
                style={{
                  "grid-column": String(index() + 2),
                }}
                class="plan-timeline-day plan-pipeline-step"
                data-plan-step={String(index() + 1)}
              >
                <strong>步骤{index() + 1}</strong>
              </span>
            )}
          </For>
        </div>
        <Show when={queuedRows().length === 0}>
          <div class="plan-pipeline-empty">{t("notScheduled")}</div>
        </Show>
        <For each={queuedRows()}>
          {(row) => {
            const session = row.session;
            return (
              <div
                class="plan-timeline-row plan-pipeline-row"
                data-session-id={session.id}
                style={{
                  "--task-count": String(row.tasks.length),
                }}
              >
                <button
                  type="button"
                  class={classNames(
                    "plan-timeline-session",
                    props.selectedSessionId === session.id &&
                      !props.selectedTaskNonceId &&
                      "selected",
                  )}
                  onClick={() => props.onOpenSession(session)}
                  title={sessionTitle(session)}
                >
                  <strong>{sessionTitle(session)}</strong>
                  <small>{shortSessionId(session.id)}</small>
                </button>
                <div
                  ref={(element) => bindTrackElement(element, session)}
                  class="plan-timeline-track plan-pipeline-track"
                  onDragOver={(event) => updateDropTarget(event, session)}
                  onDrop={(event) => dropTask(event, session)}
                  onDragLeave={() => setDropTarget(undefined)}
                >
                  <For each={row.tasks}>
                    {(task) => (
                      <button
                        ref={(element) => bindTaskElement(element, session, task)}
                        class={classNames(
                          "plan-timeline-bar",
                          "plan-pipeline-task",
                          `status-${taskPlanStatus(task) ?? planSessionStatus(session)}`,
                          dropClass(session, task),
                          props.selectedSessionId === session.id &&
                            props.selectedTaskNonceId === taskNonceId(task) &&
                            "selected",
                        )}
                        style={{
                          "grid-column": String(taskStep(task)),
                        }}
                        draggable={true}
                        onPointerDown={(event) => beginPointerDrag(event, session, task)}
                        onPointerMove={updatePointerDrag}
                        onPointerUp={finishPointerDrag}
                        onPointerCancel={() => {
                          activePointerDrag = undefined;
                          setDropTarget(undefined);
                        }}
                        onDragStart={(event) => beginTaskDrag(event, session, task)}
                        onDragEnd={() => setDropTarget(undefined)}
                        onClick={() => {
                          if (suppressTaskClick === taskNonceId(task)) {
                            return;
                          }
                          props.onEditTask(session, task);
                        }}
                        title={[
                          taskSummaryText(task) || planTaskTitle(session),
                          sessionTitle(session),
                        ].join("\n")}
                        data-task-nonce={taskNonceId(task)}
                        data-task-step={String(taskStep(task))}
                      >
                        <strong>{taskSummaryText(task) || planTaskTitle(session)}</strong>
                      </button>
                    )}
                  </For>
                </div>
              </div>
            );
          }}
        </For>
      </div>
    </section>
  );
}
