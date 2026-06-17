import asyncio
import base64
import json
import os
from pathlib import Path
from urllib.parse import urlencode

from playwright.async_api import async_playwright, expect


ROOT = Path(__file__).resolve().parents[4]
OUT = Path(
    os.environ.get(
        "TURA_GUI_PLAN_E2E_OUT",
        ROOT / "apps" / "gui" / "test-results" / "session-plan",
    )
)
GUI_URL = os.environ.get("TURA_GUI_URL", "http://127.0.0.1:5173")
VIEWPORTS = [
    {"name": "large", "width": 1920, "height": 1080},
    {"name": "small", "width": 1280, "height": 720},
    {"name": "tablet", "width": 820, "height": 1180},
    {"name": "phone", "width": 390, "height": 844},
]


async def metrics(page):
    return await page.evaluate(
        """
        () => {
          const rect = (selector) => {
            const element = document.querySelector(selector);
            if (!element) return null;
            const box = element.getBoundingClientRect();
            return {
              x: box.x,
              y: box.y,
              width: box.width,
              height: box.height,
              right: box.right,
              bottom: box.bottom,
            };
          };
          const visibleControls = Array.from(
            document.querySelectorAll('button, input, select, textarea')
          ).filter((element) => {
            const style = getComputedStyle(element);
            const box = element.getBoundingClientRect();
            return style.display !== 'none' &&
              style.visibility !== 'hidden' &&
              box.width > 0 &&
              box.height > 0;
          });
          return {
            columns: Array.from(document.querySelectorAll('.board-column')).map((column) => column.innerText),
            cards: Array.from(document.querySelectorAll('.board-card')).map((card) => card.innerText),
            boardCardShadows: Array.from(document.querySelectorAll('.board-card')).map((card) => getComputedStyle(card).boxShadow),
            boardCardCursors: Array.from(document.querySelectorAll('.board-card')).map((card) => getComputedStyle(card).cursor),
            boardAddButtons: Array.from(document.querySelectorAll('.board-column')).map((column) => ({
              text: column.querySelector('.board-column-title')?.textContent ?? '',
              buttons: column.querySelectorAll('header .icon-action.small').length,
            })),
            boardHeaderCounts: Array.from(document.querySelectorAll('.board-column header > small')).map((item) => item.textContent ?? ''),
            boardColumnScroll: Array.from(document.querySelectorAll('.board-column')).map((column) => {
              const cards = column.querySelector('.board-cards');
              const header = column.querySelector('header');
              return {
                columnOverflow: getComputedStyle(column).overflow,
                cardsOverflowY: cards ? getComputedStyle(cards).overflowY : '',
                headerTop: header ? Math.round(header.getBoundingClientRect().top) : null,
                cardsTop: cards ? Math.round(cards.getBoundingClientRect().top) : null,
              };
            }),
            archivedRows: Array.from(document.querySelectorAll('.archived-group .child-row')).map((row) => row.innerText),
            workspaceRows: Array.from(document.querySelectorAll('.workspace-row')).map((row) => row.innerText),
            activeWorkspaceRows: Array.from(document.querySelectorAll('.workspace-tree > .workspace-node:not(.archived-workspace-node) .workspace-row')).map((row) => row.innerText),
            archivedWorkspaceRows: Array.from(document.querySelectorAll('.archived-workspace-node > .workspace-row')).map((row) => row.innerText),
            archivedSectionTitles: Array.from(document.querySelectorAll('.archived-section-title')).map((row) => row.innerText),
            railSessionRows: Array.from(document.querySelectorAll('.workspace-children .session-row')).map((row) => row.innerText),
            railSessionTimeTexts: Array.from(document.querySelectorAll('.workspace-children .session-row')).map((row) => ({
              row: row.textContent ?? '',
              time: row.querySelector('.session-row-time')?.textContent ?? '',
            })),
            railSessionStatusIndicators: Array.from(document.querySelectorAll('.workspace-children .session-row .plan-status-indicator')).map((row) => row.className),
            railWorkspaceStatusIndicators: Array.from(document.querySelectorAll('.workspace-tree > .workspace-node:not(.archived-workspace-node) > .workspace-row-wrap > .workspace-row .plan-status-indicator')).map((row) => row.className),
            planTreeStatusRows: Array.from(document.querySelectorAll('.workspace-children .tree-toggle')).map((row) => row.innerText),
            archiveDrop: !!document.querySelector('.board-archive-drop'),
            archiveZone: !!document.querySelector('.board-archive-zone'),
            fileTreeRows: Array.from(document.querySelectorAll('.workspace-children .child-row')).map((row) => row.innerText),
            fileTreeLoadingRows: document.querySelectorAll('.workspace-children .file-tree-loading').length,
            splitOpen: !!document.querySelector('.plan-conversation-panel'),
            removedDraftPanels: document.querySelectorAll('.plan-draft-panel, .draft-card, .board-draft-anchor').length,
            planControls: !!document.querySelector('.plan-trigger-control'),
            composerTaskLists: document.querySelectorAll('.composer-task-list').length,
            composerTaskTexts: Array.from(document.querySelectorAll('.composer-task-list')).map((item) => item.innerText),
            composerTaskMetaTexts: Array.from(document.querySelectorAll('.composer-task-row .composer-task-meta')).map((item) => item.innerText),
            composerTaskSelectedRows: Array.from(document.querySelectorAll('.composer-task-row.selected')).map((item) => item.innerText),
            planFeedbackPrompts: Array.from(document.querySelectorAll('.plan-feedback-prompt')).map((item) => item.innerText),
            taskListInsideComposer: !!document.querySelector('.bottom-composer .composer-task-list'),
            composerText: document.querySelector('.bottom-composer textarea')?.value ?? '',
            composerAttachButtons: document.querySelectorAll('.bottom-composer .composer-attach').length,
            composerAttachHoverBg: (() => {
              const button = document.querySelector('.bottom-composer .composer-attach');
              if (!button) return '';
              const style = document.createElement('style');
              style.textContent = '.composer-attach.__e2e_hover { background: var(--wash); }';
              document.head.appendChild(style);
              button.classList.add('__e2e_hover');
              const bg = getComputedStyle(button).backgroundColor;
              button.classList.remove('__e2e_hover');
              style.remove();
              return bg;
            })(),
            composerFileInputs: document.querySelectorAll('.bottom-composer .composer-file-input[type="file"]').length,
            composerBoxes: Array.from(document.querySelectorAll('.bottom-composer')).map((item) => {
              const style = getComputedStyle(item);
              const box = item.getBoundingClientRect();
              return { boxShadow: style.boxShadow, width: box.width };
            }),
            panelClose: !!document.querySelector('.plan-panel-topbar .inspector-close'),
            resizeEdge: !!document.querySelector('.plan-conversation-panel .inspector-resize'),
            ganttRows: document.querySelectorAll('.plan-gantt .plan-timeline-row').length,
            ganttTimedBars: document.querySelectorAll('.plan-gantt .plan-timeline-bar.trigger-scheduled_task, .plan-gantt .plan-timeline-bar.trigger-polling_task').length,
            ganttWeeks: document.querySelectorAll('.plan-gantt .plan-timeline-week').length,
            ganttDayTicks: document.querySelectorAll('.plan-gantt .plan-timeline-scale [data-plan-timeline-day]').length,
            ganttRowHeights: Array.from(document.querySelectorAll('.plan-timeline-row')).map((row) => row.getBoundingClientRect().height),
            ganttBarTexts: Array.from(document.querySelectorAll('.plan-timeline-bar')).map((bar) => bar.innerText),
            ganttBarShadows: Array.from(document.querySelectorAll('.plan-timeline-bar')).map((bar) => getComputedStyle(bar).boxShadow),
            ganttBarCursors: Array.from(document.querySelectorAll('.plan-timeline-bar')).map((bar) => getComputedStyle(bar).cursor),
            ganttBarPositioning: Array.from(document.querySelectorAll('.plan-timeline-bar')).map((bar) => {
              const style = getComputedStyle(bar);
              return { position: style.position, left: style.left, gridColumn: style.gridColumnStart };
            }),
            ganttHeadText: document.querySelector('.plan-timeline-left-head')?.innerText ?? '',
            ganttLeftAxisTexts: Array.from(document.querySelectorAll('.plan-timeline-row > span')).map((axis) => axis.innerText),
            ganttDayWidth: document.querySelector('.plan-timeline-scale [data-plan-timeline-day]')?.getBoundingClientRect().width ?? 0,
            ganttFirstDay: document.querySelector('.plan-timeline-scale [data-plan-timeline-day]')?.getAttribute('data-plan-timeline-day') ?? '',
            ganttCanScrollX: (() => {
              const grid = document.querySelector('.plan-timeline-grid');
              return grid ? grid.scrollWidth > grid.clientWidth + 1 : false;
            })(),
            ganttVisibleDays: (() => {
              const days = Array.from(document.querySelectorAll('.plan-timeline-scale > [data-plan-timeline-day]'));
              const first = days[0]?.getBoundingClientRect();
              const last = days.at(-1)?.getBoundingClientRect();
              if (!first || !last || first.width <= 0) return 0;
              return (last.right - first.left) / first.width;
            })(),
            ganttScrollLeft: document.querySelector('.plan-timeline-grid')?.scrollLeft ?? 0,
            calendarRows: document.querySelectorAll('.plan-calendar .plan-calendar-cell').length,
            calendarEvents: document.querySelectorAll('.plan-calendar .plan-calendar-event').length,
            calendarEventShadows: Array.from(document.querySelectorAll('.plan-calendar .plan-calendar-event')).map((event) => getComputedStyle(event).boxShadow),
            calendarEventCursors: Array.from(document.querySelectorAll('.plan-calendar .plan-calendar-event')).map((event) => getComputedStyle(event).cursor),
            calendarEventTops: Array.from(document.querySelectorAll('.plan-calendar .plan-calendar-cell'))
              .map((cell) => {
                const event = cell.querySelector('.plan-calendar-event');
                return event ? Math.round(event.getBoundingClientRect().top - cell.getBoundingClientRect().top) : null;
              })
              .filter((value) => value !== null),
            calendarHeaderHeights: Array.from(document.querySelectorAll('.plan-calendar .plan-calendar-cell header')).map((header) => Math.round(header.getBoundingClientRect().height)),
            calendarWeekRows: document.querySelectorAll('.plan-calendar-hour-cell').length,
            calendarTimedEvents: document.querySelectorAll('.plan-calendar .plan-calendar-event.trigger-scheduled_task, .plan-calendar .plan-calendar-event.trigger-polling_task').length,
            calendarPollingEvents: document.querySelectorAll('.plan-calendar .plan-calendar-event.trigger-polling_task').length,
            calendarTitle: document.querySelector('.plan-calendar-title strong')?.innerText ?? '',
            calendarHeaderText: document.querySelector('.plan-calendar-title')?.innerText ?? '',
            calendarNavButtons: document.querySelectorAll('.plan-calendar-nav .icon-action').length,
            calendarViewButtons: document.querySelectorAll('.plan-calendar-view-toggle button').length,
            calendarMonthSelected: !!document.querySelector('.plan-calendar-view-toggle button.selected:first-child'),
            calendarWeekSelected: Array.from(document.querySelectorAll('.plan-calendar-view-toggle button.selected')).some((button) => button.innerText.includes('周')),
            calendarSelectedWeekDay: !!document.querySelector('.plan-calendar-week-day.selected'),
            draftSessionPicker: !!document.querySelector('.plan-session-picker'),
            draftSessionMenu: !!document.querySelector('.plan-session-menu'),
            draftSessionRows: Array.from(document.querySelectorAll('.plan-session-menu .workspace-pick-row')).map((row) => row.innerText),
            draftSessionRowWidths: Array.from(document.querySelectorAll('.plan-session-menu .workspace-pick-row span')).map((row) => Math.round(row.getBoundingClientRect().width)),
            triggerButtonText: document.querySelector('.plan-trigger-button')?.innerText ?? '',
            triggerIconOnly: !!document.querySelector('.plan-trigger-button svg') &&
              !document.querySelector('.plan-trigger-button span')?.offsetParent,
            scheduleIntervalLabels: Array.from(document.querySelectorAll('.plan-schedule-dialog .plan-schedule-interval-grid span')).map((item) => item.textContent ?? ''),
            scheduleIntervalMaxLengths: Array.from(document.querySelectorAll('.plan-schedule-dialog .plan-schedule-interval-grid input')).map((item) => item.getAttribute('maxlength')),
            scheduleCloseBox: rect('.plan-schedule-dialog header button'),
            scheduleTitleBox: rect('.plan-schedule-dialog h2'),
            error: document.querySelector('.error-strip')?.innerText ?? '',
            overflowX: document.documentElement.scrollWidth - document.documentElement.clientWidth,
            bodyOverflowX: document.body.scrollWidth - document.body.clientWidth,
            modeButtons: document.querySelectorAll('.plan-mode-actions .icon-action').length,
            selectedModes: document.querySelectorAll('.plan-mode-actions .icon-action.selected').length,
            panelTitle: document.querySelector('.plan-panel-title')?.innerText ?? '',
            ticketPanel: !!document.querySelector('.plan-trigger-control'),
            railVisible: !!document.querySelector('.rail') &&
              getComputedStyle(document.querySelector('.rail')).display !== 'none',
            board: rect('.plan-board'),
            main: rect('.plan-main'),
            panel: rect('.plan-conversation-panel'),
            workbench: rect('.plan-workbench'),
            head: rect('.page-head'),
            modeActions: rect('.plan-mode-actions'),
            offscreenControls: visibleControls.filter((element) => {
              if (element.closest('.plan-board')) return false;
              const box = element.getBoundingClientRect();
              return box.right < 0 ||
                box.left > innerWidth ||
                box.bottom < 0 ||
                box.top > innerHeight;
            }).length,
            tinyControls: visibleControls.filter((element) => {
              const box = element.getBoundingClientRect();
              return box.width < 24 || box.height < 24;
            }).length,
          };
        }
        """
    )


def add_result(results, viewport, name, ok, detail=None):
    result = {"name": f"{viewport}:{name}", "ok": bool(ok)}
    if detail is not None:
        result["detail"] = detail
    results.append(result)


def enough_main_width(data, viewport_width):
    main = data.get("main")
    if not main:
        return False
    minimum = min(360, viewport_width - 32)
    return main["width"] >= minimum


def panel_is_usable(data, viewport_width):
    panel = data.get("panel")
    if not panel:
        return False
    minimum = min(360, viewport_width)
    return panel["width"] >= minimum and panel["height"] >= 320


async def drag_ticket_to_column(page, ticket_text, column_text):
    await page.evaluate(
        """
        ({ ticketText, columnText }) => {
          const card = Array.from(document.querySelectorAll('.board-card'))
            .find((item) => item.innerText.includes(ticketText));
          const column = Array.from(document.querySelectorAll('.board-column'))
            .find((item) => item.innerText.includes(columnText));
          if (!card || !column) throw new Error('drag target not found');
          const dataTransfer = new DataTransfer();
          card.dispatchEvent(new DragEvent('dragstart', { bubbles: true, dataTransfer }));
          column.dispatchEvent(new DragEvent('dragover', { bubbles: true, dataTransfer }));
          column.dispatchEvent(new DragEvent('drop', { bubbles: true, dataTransfer }));
        }
        """,
        {"ticketText": ticket_text, "columnText": column_text},
    )


async def drag_plan_item(page, source_selector, target_selector, source_text=None, target_index=0):
    await page.evaluate(
        """
        ({ sourceSelector, targetSelector, sourceText, targetIndex }) => {
          const sources = Array.from(document.querySelectorAll(sourceSelector));
          const source = sourceText
            ? sources.find((item) => item.innerText.includes(sourceText))
            : sources[0];
          const targets = Array.from(document.querySelectorAll(targetSelector));
          const target = targets[targetIndex] ?? targets[0];
          if (!source || !target) {
            throw new Error(JSON.stringify({
              message: 'plan drag target not found',
              sourceSelector,
              targetSelector,
              sourceText,
              sourceCount: sources.length,
              targetCount: targets.length,
              sourceTexts: sources.map((item) => item.innerText).slice(0, 6),
              calendarTitle: document.querySelector('.plan-calendar-title')?.innerText ?? '',
            }));
          }
          const dataTransfer = new DataTransfer();
          source.dispatchEvent(new DragEvent('dragstart', { bubbles: true, dataTransfer }));
          target.dispatchEvent(new DragEvent('dragover', { bubbles: true, dataTransfer }));
          target.dispatchEvent(new DragEvent('drop', { bubbles: true, dataTransfer }));
        }
        """,
        {
            "sourceSelector": source_selector,
            "targetSelector": target_selector,
            "sourceText": source_text,
            "targetIndex": target_index,
        },
    )


async def pointer_drag_between(page, source, target, x_ratio=0.5, y_ratio=0.5):
    await source.scroll_into_view_if_needed()
    await target.scroll_into_view_if_needed()
    source_box = await source.bounding_box()
    target_box = await target.bounding_box()
    if not source_box or not target_box:
        raise AssertionError("pointer drag boxes not found")
    start_x = source_box["x"] + source_box["width"] / 2
    start_y = source_box["y"] + source_box["height"] / 2
    end_x = target_box["x"] + target_box["width"] * x_ratio
    end_y = target_box["y"] + target_box["height"] * y_ratio
    await page.mouse.move(start_x, start_y)
    await page.mouse.down()
    await page.mouse.move(end_x, end_y, steps=12)
    await page.wait_for_timeout(50)
    ghost = await page.evaluate(
        """
        ({ x, y }) => {
          const ghost = document.querySelector('.plan-drag-ghost');
          if (!ghost) return null;
          const box = ghost.getBoundingClientRect();
          const source = document.querySelector('.plan-source-dragging');
          const sourceBox = source?.getBoundingClientRect();
            return {
            left: box.left,
            top: box.top,
            centerX: box.left + box.width / 2,
            centerY: box.top + box.height / 2,
            pointerX: x,
            pointerY: y,
            delta: Math.hypot(box.left + box.width / 2 - x, box.top + box.height / 2 - y),
            topLeftDelta: Math.hypot(box.left - x, box.top - y),
            pointerInside: x >= box.left && x <= box.right && y >= box.top && y <= box.bottom,
            sameWidth: sourceBox ? Math.abs(box.width - sourceBox.width) <= 1 : false,
            sameHeight: sourceBox ? Math.abs(box.height - sourceBox.height) <= 1 : false,
            ghostClass: ghost.className,
            sourceHidden: !!source && getComputedStyle(source).visibility === 'hidden',
            sourceCursor: source ? getComputedStyle(source).cursor : null,
          };
        }
        """,
        {"x": end_x, "y": end_y},
    )
    await page.mouse.up()
    return ghost


async def pointer_drag_by_offset(page, source, dx=24, dy=12):
    source_box = None
    for _ in range(3):
        try:
            await source.scroll_into_view_if_needed()
            source_box = await source.bounding_box()
            if source_box:
                break
        except Exception:
            await page.wait_for_timeout(100)
    if not source_box:
        raise AssertionError("pointer drag source box not found")
    start_x = source_box["x"] + source_box["width"] / 2
    start_y = source_box["y"] + source_box["height"] / 2
    end_x = start_x + dx
    end_y = start_y + dy
    await page.mouse.move(start_x, start_y)
    await page.mouse.down()
    await page.mouse.move(end_x, end_y, steps=6)
    await page.wait_for_timeout(50)
    ghost = await page.evaluate(
        """
        ({ x, y }) => {
          const ghost = document.querySelector('.plan-drag-ghost');
          if (!ghost) return null;
          const box = ghost.getBoundingClientRect();
          const source = document.querySelector('.plan-source-dragging');
          const sourceBox = source?.getBoundingClientRect();
          return {
            left: box.left,
            top: box.top,
            centerX: box.left + box.width / 2,
            centerY: box.top + box.height / 2,
            pointerX: x,
            pointerY: y,
            delta: Math.hypot(box.left + box.width / 2 - x, box.top + box.height / 2 - y),
            topLeftDelta: Math.hypot(box.left - x, box.top - y),
            pointerInside: x >= box.left && x <= box.right && y >= box.top && y <= box.bottom,
            sameWidth: sourceBox ? Math.abs(box.width - sourceBox.width) <= 1 : false,
            sameHeight: sourceBox ? Math.abs(box.height - sourceBox.height) <= 1 : false,
            ghostClass: ghost.className,
            sourceHidden: !!source && getComputedStyle(source).visibility === 'hidden',
            sourceCursor: source ? getComputedStyle(source).cursor : null,
          };
        }
        """,
        {"x": end_x, "y": end_y},
    )
    await page.mouse.up()
    return ghost


async def pointer_small_move_state(page, source, dx=3, dy=2):
    source_box = None
    for _ in range(3):
        try:
            await source.scroll_into_view_if_needed()
            source_box = await source.bounding_box()
            if source_box:
                break
        except Exception:
            await page.wait_for_timeout(100)
    if not source_box:
        raise AssertionError("pointer small move source box not found")
    start_x = source_box["x"] + source_box["width"] / 2
    start_y = source_box["y"] + source_box["height"] / 2
    await page.mouse.move(start_x, start_y)
    await page.mouse.down()
    await page.mouse.move(start_x + dx, start_y + dy, steps=3)
    await page.wait_for_timeout(50)
    state = await page.evaluate(
        """
        () => ({
          ghostCount: document.querySelectorAll('.plan-drag-ghost').length,
          sourceDraggingCount: document.querySelectorAll('.plan-source-dragging').length,
        })
        """
    )
    await page.mouse.up()
    return state


async def trigger_menu_geometry(page, scope_selector):
    return await page.evaluate(
        """
        (scopeSelector) => {
          const scope = document.querySelector(scopeSelector);
          const button = scope?.querySelector('.plan-trigger-button');
          const menu = scope?.querySelector('.plan-trigger-menu');
          if (!scope || !button || !menu) return null;
          const buttonBox = button.getBoundingClientRect();
          const menuBox = menu.getBoundingClientRect();
          return {
            buttonTop: buttonBox.top,
            buttonLeft: buttonBox.left,
            buttonCenter: buttonBox.left + buttonBox.width / 2,
            buttonWidth: buttonBox.width,
            buttonHeight: buttonBox.height,
            menuTop: menuBox.top,
            menuLeft: menuBox.left,
            menuBottom: menuBox.bottom,
            menuCenter: menuBox.left + menuBox.width / 2,
            leftDelta: Math.abs(buttonBox.left - menuBox.left),
            centerDelta: Math.abs(
              buttonBox.left + buttonBox.width / 2 - (menuBox.left + menuBox.width / 2)
            ),
            gap: buttonBox.top - menuBox.bottom,
            visible:
              menuBox.top >= 0 &&
              menuBox.left >= 0 &&
              menuBox.right <= innerWidth &&
              menuBox.bottom <= innerHeight,
            toolbarButtonHeights: Array.from(
              scope.querySelectorAll('.composer-attach, .plan-trigger-button, .composer-send')
            ).map((item) => item.getBoundingClientRect().height),
            menuOptionHeights: Array.from(
              scope.querySelectorAll('.plan-trigger-option')
            ).map((item) => item.getBoundingClientRect().height),
            topElement: (() => {
              const element = document.elementFromPoint(
                menuBox.left + menuBox.width / 2,
                menuBox.top + 8
              );
              return element ? String(element.className) : '';
            })(),
            bottomElement: (() => {
              const element = document.elementFromPoint(
                menuBox.left + menuBox.width / 2,
                menuBox.bottom - 8
              );
              return element ? String(element.className) : '';
            })(),
          };
        }
        """,
        scope_selector,
    )


async def check_trigger_menu(page, results, viewport, scope_selector, name):
    button = page.locator(
        f"{scope_selector} .plan-trigger-control:not(.agent-trigger-control) .plan-trigger-button"
    )
    await expect(button).to_be_visible()
    await button.click()
    await expect(page.locator(f"{scope_selector} .plan-trigger-menu")).to_be_visible()
    geometry = await trigger_menu_geometry(page, scope_selector)
    add_result(
        results,
        viewport,
        f"{name}-trigger-menu-left-aligned-above-button",
        bool(
            geometry
            and geometry["visible"]
            and geometry["leftDelta"] <= 4
            and -1 <= geometry["gap"] <= 1
            and all(abs(height - 34) <= 1 for height in geometry["toolbarButtonHeights"])
            and all(abs(height - 34) <= 1 for height in geometry["menuOptionHeights"])
            and "plan-trigger" in geometry["topElement"]
            and "plan-trigger" in geometry["bottomElement"]
        ),
        geometry,
    )
    add_result(
        results,
        viewport,
        f"{name}-trigger-menu-has-run-now",
        await page.locator(f"{scope_selector} .plan-trigger-option").filter(has_text="立刻执行").count()
        == 1,
    )
    add_result(
        results,
        viewport,
        f"{name}-trigger-selected-mode-has-check",
        await page.locator(f"{scope_selector} .plan-trigger-option.selected svg").count() == 1,
    )
    await page.mouse.click(6, 6)
    await page.wait_for_timeout(60)
    add_result(
        results,
        viewport,
        f"{name}-trigger-menu-closes-on-outside-click",
        await page.locator(f"{scope_selector} .plan-trigger-menu").count() == 0,
    )


async def ticket_backgrounds(page, selector):
    return await page.evaluate(
        """
        (selector) => Array.from(document.querySelectorAll(selector))
          .map((item) => getComputedStyle(item).backgroundColor)
        """,
        selector,
    )


async def close_plan_panel(page):
    close_button = page.locator(".plan-panel-topbar .inspector-close")
    if await close_button.count():
        await close_button.first.click()
        await expect(page.locator(".plan-conversation-panel")).to_have_count(0)
        await page.wait_for_timeout(50)


async def drag_resize_to_close(page, width):
    handle = page.locator(".plan-conversation-panel .inspector-resize").first
    box = await handle.bounding_box()
    if not box:
        return False
    y = box["y"] + max(12, min(36, box["height"] / 2))
    await page.mouse.move(box["x"] + box["width"] / 2, y)
    await page.mouse.down()
    await page.mouse.move(width - 2, y, steps=5)
    await page.mouse.up()
    await page.wait_for_timeout(150)
    return not await page.locator(".plan-conversation-panel").count()


async def run_plan_flow(browser, viewport, image_path, attachment_path, browser_errors):
    name = viewport["name"]
    width = viewport["width"]
    results = []
    page = await browser.new_page(
        viewport={"width": viewport["width"], "height": viewport["height"]}
    )
    page.on(
        "console",
        lambda message: browser_errors.append(
            {"viewport": name, "kind": message.type, "text": message.text}
        )
        if message.type in {"error", "warning"}
        else None,
    )
    page.on(
        "pageerror",
        lambda error: browser_errors.append(
            {"viewport": name, "kind": "pageerror", "text": str(error)}
        ),
    )

    await page.goto(
        f"{GUI_URL}/?{urlencode({'e2eFixture': 'plan-sessions'})}",
        wait_until="domcontentloaded",
    )
    await page.wait_for_selector(".plan-board .board-card", timeout=30_000)
    await page.screenshot(path=str(OUT / f"{name}-initial.png"), full_page=True)

    first = await metrics(page)
    add_result(results, name, "four-plan-columns", len(first["columns"]) == 4)
    add_result(results, name, "four-mode-buttons", first["modeButtons"] == 4)
    add_result(results, name, "one-active-mode", first["selectedModes"] == 1)
    add_result(
        results,
        name,
        "archived-hidden-from-board",
        "隐藏旧会话工单" not in "\n".join(first["cards"]),
    )
    add_result(
        results,
        name,
        "other-workspace-hidden-from-board",
        "其他目录里的待办" not in "\n".join(first["cards"]),
    )
    add_result(
        results,
        name,
        "plan-left-rail-hides-other-workspace",
        first["activeWorkspaceRows"].count("tura") == 1 and all("other" not in row for row in first["activeWorkspaceRows"]),
        {"active": first["activeWorkspaceRows"], "all": first["workspaceRows"]},
    )
    add_result(
        results,
        name,
        "plan-left-rail-has-status-groups-and-separate-archived-sessions",
        all(label in "\n".join(first["planTreeStatusRows"]) for label in ["待办", "进行中", "待反馈", "完成"])
        and "已归档" not in "\n".join(first["planTreeStatusRows"])
        and first["archivedSectionTitles"]
        and first["archivedWorkspaceRows"]
        and not first["archiveDrop"]
        and first["archiveZone"],
        {
            "tree": first["planTreeStatusRows"],
            "archivedTitles": first["archivedSectionTitles"],
            "archivedWorkspaces": first["archivedWorkspaceRows"],
            "archiveDrop": first["archiveDrop"],
            "archiveZone": first["archiveZone"],
        },
    )
    add_result(
        results,
        name,
        "todo-ticket-visible",
        any("整理发布检查清单" in card for card in first["cards"]),
    )
    add_result(results, name, "no-document-overflow", first["overflowX"] <= 1)
    add_result(results, name, "no-body-overflow", first["bodyOverflowX"] <= 1)
    add_result(results, name, "main-keeps-mobile-min-width", enough_main_width(first, width), first)
    add_result(results, name, "no-offscreen-controls", first["offscreenControls"] == 0)
    add_result(results, name, "no-tiny-controls", first["tinyControls"] == 0)
    add_result(
        results,
        name,
        "todo-board-tickets-have-subtle-shadow",
        first["boardCardShadows"] and all("rgba(0, 0, 0, 0.08)" in shadow or "rgb(0 0 0 / 0.08)" in shadow for shadow in first["boardCardShadows"]),
        first["boardCardShadows"],
    )
    add_result(
        results,
        name,
        "todo-board-user-action-hides-time",
        any("用户操作不显示在日历" in card and "2026" not in card and "/" not in card for card in first["cards"]),
        first["cards"],
    )
    add_result(
        results,
        name,
        "todo-board-only-todo-column-has-create-button",
        first["boardAddButtons"]
        and first["boardAddButtons"][0]["buttons"] == 1
        and all(item["buttons"] == 0 for item in first["boardAddButtons"][1:]),
        first["boardAddButtons"],
    )
    add_result(
        results,
        name,
        "todo-board-column-counts-are-hidden",
        first["boardHeaderCounts"] == [],
        first["boardHeaderCounts"],
    )
    add_result(
        results,
        name,
        "todo-board-columns-scroll-independently-under-sticky-headers",
        first["boardColumnScroll"]
        and all(item["columnOverflow"] == "hidden" for item in first["boardColumnScroll"])
        and all(item["cardsOverflowY"] in ["auto", "scroll"] for item in first["boardColumnScroll"])
        and all(item["cardsTop"] is not None and item["headerTop"] is not None and item["cardsTop"] > item["headerTop"] for item in first["boardColumnScroll"]),
        first["boardColumnScroll"],
    )
    board_drag_state = await page.evaluate(
        """
        () => {
          const card = Array.from(document.querySelectorAll('.board-card'))
            .find((item) => item.innerText.includes('整理发布检查清单'));
          if (!card) return null;
          const dataTransfer = new DataTransfer();
          card.dispatchEvent(new DragEvent('dragstart', { bubbles: true, dataTransfer }));
          const state = {
            hidden: getComputedStyle(card).visibility === 'hidden',
            cursor: getComputedStyle(card).cursor,
          };
          card.dispatchEvent(new DragEvent('dragend', { bubbles: true, dataTransfer }));
          return state;
        }
        """
    )
    add_result(
        results,
        name,
        "todo-board-source-hides-while-dragging",
        bool(board_drag_state and board_drag_state["hidden"] and board_drag_state["cursor"] == "pointer"),
        board_drag_state,
    )
    board_small_move = await pointer_small_move_state(
        page,
        page.locator(".board-card").filter(has_text="整理发布检查清单").first,
    )
    add_result(
        results,
        name,
        "todo-board-small-pointer-move-does-not-start-drag",
        board_small_move["ghostCount"] == 0
        and board_small_move["sourceDraggingCount"] == 0,
        board_small_move,
    )
    await close_plan_panel(page)

    await page.goto(
        f"{GUI_URL}/?{urlencode({'e2eFixture': 'plan-sessions', 'tab': 'files'})}",
        wait_until="domcontentloaded",
    )
    await page.wait_for_selector(".files-view", timeout=30_000)
    files_first = await metrics(page)
    if files_first["railVisible"]:
        add_result(
            results,
            name,
            "files-rail-shows-directories-and-files",
            any("apps/" in row for row in files_first["fileTreeRows"])
            and any("README.md" in row for row in files_first["fileTreeRows"])
            and any("package.json" in row for row in files_first["fileTreeRows"]),
            files_first["fileTreeRows"],
        )
        await page.locator(".workspace-children .child-row").filter(has_text="apps/").click()
        await page.wait_for_timeout(150)
        files_after_dir = await metrics(page)
        add_result(
            results,
            name,
            "files-rail-keeps-sibling-directions-when-directory-loads",
            any("apps/" in row for row in files_after_dir["fileTreeRows"])
            and any("gui/" in row for row in files_after_dir["fileTreeRows"])
            and any("crates/" in row for row in files_after_dir["fileTreeRows"])
            and any("README.md" in row for row in files_after_dir["fileTreeRows"]),
            files_after_dir["fileTreeRows"],
        )
    else:
        add_result(results, name, "files-page-opens-on-compact-layout", files_first["railVisible"] is False)
    await page.evaluate(
        """
        () => {
          const button = Array.from(document.querySelectorAll('.main-tabs button'))
            .find((item) => item.innerText.includes('计划'));
          if (!button) throw new Error('plan tab not found');
          button.click();
        }
        """
    )
    await page.wait_for_selector(".plan-board .board-card", timeout=30_000)

    await page.goto(
        f"{GUI_URL}/?{urlencode({'e2eFixture': 'plan-sessions', 'tab': 'new'})}",
        wait_until="domcontentloaded",
    )
    await page.wait_for_selector(".new-session-view .bottom-composer .plan-trigger-control", timeout=30_000)
    await check_trigger_menu(page, results, name, ".new-session-view .bottom-composer", "new-session-composer")

    await page.goto(
        f"{GUI_URL}/?{urlencode({'e2eFixture': 'plan-sessions', 'tab': 'conversation'})}",
        wait_until="domcontentloaded",
    )
    await page.wait_for_selector(".conversation-view:not(.compact) .plan-trigger-control", timeout=30_000)
    conversation_task_metrics = await metrics(page)
    add_result(
        results,
        name,
        "conversation-composer-shows-session-task-list-and-attachment-input",
        conversation_task_metrics["composerTaskLists"] == 1
        and not conversation_task_metrics["taskListInsideComposer"]
        and conversation_task_metrics["composerAttachButtons"] == 1
        and conversation_task_metrics["composerFileInputs"] == 1
        and any("整理发布检查清单" in text for text in conversation_task_metrics["composerTaskTexts"])
        and not any("执行状态" in text for text in conversation_task_metrics["composerTaskTexts"])
        and not conversation_task_metrics["planFeedbackPrompts"],
        conversation_task_metrics,
    )
    add_result(
        results,
        name,
        "composer-attach-button-has-hover-feedback",
        conversation_task_metrics["composerAttachHoverBg"] not in ["", "rgba(0, 0, 0, 0)", "transparent"],
        conversation_task_metrics["composerAttachHoverBg"],
    )
    add_result(
        results,
        name,
        "conversation-rail-session-status-replaces-time",
        any("status-doing" in item for item in conversation_task_metrics["railSessionStatusIndicators"])
        and any("status-question" in item for item in conversation_task_metrics["railSessionStatusIndicators"])
        and any("status-done" in item for item in conversation_task_metrics["railSessionStatusIndicators"]),
        conversation_task_metrics["railSessionStatusIndicators"],
    )
    if conversation_task_metrics["railVisible"]:
        await page.locator(".workspace-children .session-row").filter(has_text="完成 gateway 字段回传").click()
        await page.wait_for_timeout(150)
        done_ack_metrics = await metrics(page)
        add_result(
            results,
            name,
            "conversation-rail-clicking-done-acknowledges-status",
            not any("status-done" in item for item in done_ack_metrics["railSessionStatusIndicators"])
            and any(
                "完成 gateway 字段回传" in item["row"] and item["time"]
                for item in done_ack_metrics["railSessionTimeTexts"]
            ),
            {
                "rows": done_ack_metrics["railSessionRows"],
                "times": done_ack_metrics["railSessionTimeTexts"],
                "status": done_ack_metrics["railSessionStatusIndicators"],
            },
        )
        await page.locator(".workspace-tree > .workspace-node:not(.archived-workspace-node) > .workspace-row-wrap > .workspace-row").click()
        await page.wait_for_timeout(100)
        collapsed_workspace_metrics = await metrics(page)
        add_result(
            results,
            name,
            "collapsed-workspace-shows-latest-active-status",
            any("status-doing" in item or "status-question" in item for item in collapsed_workspace_metrics["railWorkspaceStatusIndicators"]),
            collapsed_workspace_metrics["railWorkspaceStatusIndicators"],
        )
        await page.locator(".workspace-tree > .workspace-node:not(.archived-workspace-node) > .workspace-row-wrap > .workspace-row").click()
        await page.wait_for_timeout(100)
    await check_trigger_menu(
        page,
        results,
        name,
        ".conversation-view:not(.compact)",
        "conversation-composer",
    )

    await page.goto(
        f"{GUI_URL}/?{urlencode({'e2eFixture': 'plan-sessions'})}",
        wait_until="domcontentloaded",
    )
    await page.wait_for_selector(".plan-board .board-card", timeout=30_000)
    await page.locator(".board-card").filter(has_text="整理发布检查清单").first.click(force=True)
    await page.wait_for_timeout(150)
    panel_task_metrics = await metrics(page)
    add_result(
        results,
        name,
        "plan-side-composer-reuses-task-list-and-adds-session-picker",
        panel_task_metrics["splitOpen"]
        and panel_task_metrics["composerTaskLists"] == 1
        and panel_task_metrics["composerAttachButtons"] == 1
        and panel_task_metrics["composerFileInputs"] == 1
        and not panel_task_metrics["taskListInsideComposer"]
        and panel_task_metrics["draftSessionPicker"] is False
        and any("整理发布检查清单" in text for text in panel_task_metrics["composerTaskTexts"])
        and not any("执行状态" in text for text in panel_task_metrics["composerTaskTexts"])
        and any(("天" in text or "时" in text or "分" in text or "秒" in text) for text in panel_task_metrics["composerTaskMetaTexts"])
        and not panel_task_metrics["planFeedbackPrompts"],
        panel_task_metrics,
    )
    await page.locator(".plan-conversation-panel .composer-task-row").click()
    await page.wait_for_timeout(120)
    task_after_click = await metrics(page)
    add_result(
        results,
        name,
        "plan-composer-task-list-click-fills-input-for-editing",
        task_after_click["composerText"] == "整理发布检查清单\n\nsession ticket e2e"
        and any("整理发布检查清单" in row for row in task_after_click["composerTaskSelectedRows"]),
        task_after_click,
    )
    await close_plan_panel(page)
    await page.locator(".board-column").filter(has_text="待办").locator(".icon-action").click()
    await page.wait_for_timeout(150)
    draft_task_metrics = await metrics(page)
    add_result(
        results,
        name,
        "plan-draft-composer-only-adds-session-picker-to-shared-composer",
        draft_task_metrics["splitOpen"]
        and draft_task_metrics["draftSessionPicker"]
        and draft_task_metrics["composerTaskLists"] == 0
        and draft_task_metrics["composerAttachButtons"] == 1
        and draft_task_metrics["composerFileInputs"] == 1
        and not draft_task_metrics["planFeedbackPrompts"],
        draft_task_metrics,
    )
    await close_plan_panel(page)

    await page.get_by_role("button", name="甘特图", exact=True).click(force=True)
    await page.wait_for_timeout(100)
    gantt = await metrics(page)
    add_result(
        results,
        name,
        "gantt-mode-renders-only-timed-tasks",
        gantt["ganttRows"] == 2
        and gantt["ganttTimedBars"] == 2
        and gantt["ganttWeeks"] == 0
        and gantt["ganttDayTicks"] == 7
        and not gantt["ganttCanScrollX"],
        gantt,
    )
    add_result(
        results,
        name,
        "gantt-head-is-single-date-row-without-extra-title",
        gantt["ganttHeadText"].strip() == ""
        and gantt["ganttVisibleDays"] <= 7.05
        and gantt["ganttDayWidth"] >= 24,
        {
            "head": gantt["ganttHeadText"],
            "visibleDays": gantt["ganttVisibleDays"],
            "dayWidth": gantt["ganttDayWidth"],
            "canScrollX": gantt["ganttCanScrollX"],
        },
    )
    add_result(
        results,
        name,
        "gantt-rows-have-breathing-room-and-align",
        all(44 <= height <= 52 for height in gantt["ganttRowHeights"]),
        gantt["ganttRowHeights"],
    )
    add_result(
        results,
        name,
        "gantt-axis-shows-session-and-task",
        all("\n" in text and "执行状态" not in text for text in gantt["ganttLeftAxisTexts"]),
        gantt["ganttLeftAxisTexts"],
    )
    add_result(
        results,
        name,
        "gantt-ticket-hides-time",
        all(":" not in text and "/" not in text for text in gantt["ganttBarTexts"]),
        gantt["ganttBarTexts"],
    )
    gantt_backgrounds = await ticket_backgrounds(page, ".plan-timeline-bar")
    add_result(
        results,
        name,
        "gantt-tickets-do-not-add-status-colors",
        len(set(gantt_backgrounds)) <= 1,
        gantt_backgrounds,
    )
    add_result(
        results,
        name,
        "gantt-tickets-have-subtle-shadow-and-default-cursor",
        gantt["ganttBarShadows"]
        and all("rgba(0, 0, 0, 0.08)" in shadow or "rgb(0 0 0 / 0.08)" in shadow for shadow in gantt["ganttBarShadows"])
        and all(cursor == "default" for cursor in gantt["ganttBarCursors"]),
        {"shadows": gantt["ganttBarShadows"], "cursors": gantt["ganttBarCursors"]},
    )
    add_result(
        results,
        name,
        "gantt-tickets-use-continuous-time-position",
        gantt["ganttBarPositioning"]
        and all(item["position"] == "absolute" and item["left"] not in ("auto", "0px") and item["gridColumn"] == "auto" for item in gantt["ganttBarPositioning"]),
        gantt["ganttBarPositioning"],
    )
    add_result(
        results,
        name,
        "gantt-hides-user-action-tasks",
        await page.locator(".plan-gantt").filter(has_text="用户操作不显示在日历").count() == 0,
    )
    await pointer_drag_between(
        page,
        page.locator(".plan-timeline-bar").filter(has_text="整理发布检查清单").first,
        page.locator(".plan-timeline-drop").nth(5),
        x_ratio=0.74,
        y_ratio=0.5,
    )
    await page.wait_for_timeout(150)
    add_result(
        results,
        name,
        "gantt-ticket-drags-to-date",
        await page.locator(".plan-timeline-bar").filter(has_text="整理发布检查清单").count() == 1,
    )
    await page.get_by_role("button", name="待办列表", exact=True).click(force=True)
    await page.wait_for_timeout(100)
    gantt_precise_card = await page.locator(".board-card").filter(has_text="整理发布检查清单").first.inner_text()
    add_result(
        results,
        name,
        "gantt-drag-uses-pointer-minute-precision",
        "整理发布检查清单" in gantt_precise_card
        and ":" in gantt_precise_card
        and "00:00" not in gantt_precise_card,
        gantt_precise_card,
    )
    await page.goto(f"{GUI_URL}?e2eFixture=plan-sessions")
    await page.wait_for_load_state("domcontentloaded")
    await page.wait_for_timeout(250)
    await page.get_by_role("button", name="甘特图", exact=True).click(force=True)
    await page.wait_for_timeout(100)
    gantt_small_move = await pointer_small_move_state(
        page,
        page.locator(".plan-timeline-bar").filter(has_text="整理发布检查清单").first,
    )
    add_result(
        results,
        name,
        "gantt-small-pointer-move-does-not-start-drag",
        gantt_small_move["ghostCount"] == 0
        and gantt_small_move["sourceDraggingCount"] == 0,
        gantt_small_move,
    )
    await close_plan_panel(page)
    gantt_drag_ghost = await pointer_drag_by_offset(
        page,
        page.locator(".plan-timeline-bar").filter(has_text="整理发布检查清单").first,
    )
    add_result(
        results,
        name,
        "gantt-drag-moves-ticket-itself-and-keeps-default-cursor",
        bool(
            gantt_drag_ghost
            and gantt_drag_ghost["sourceHidden"]
            and gantt_drag_ghost["sourceCursor"] == "default"
            and gantt_drag_ghost["pointerInside"]
            and gantt_drag_ghost["topLeftDelta"] <= 2
            and gantt_drag_ghost["sameWidth"]
            and gantt_drag_ghost["sameHeight"]
            and "plan-timeline-bar" in gantt_drag_ghost["ghostClass"]
        ),
        gantt_drag_ghost,
    )
    before_pan = (await metrics(page))["ganttFirstDay"]
    scale_box = await page.locator(".plan-timeline-scale").bounding_box()
    grid_box = await page.locator(".plan-timeline-grid").bounding_box()
    scale_y = (scale_box["y"] + scale_box["height"] / 2) if scale_box else 180
    start_x = min(width - 96, (grid_box["x"] + grid_box["width"] - 120) if grid_box else width - 96)
    end_x = max(32, start_x - min(56, width * 0.08))
    await page.mouse.move(start_x, scale_y)
    await page.mouse.down()
    await page.mouse.move(end_x, scale_y, steps=8)
    await page.mouse.up()
    await page.wait_for_timeout(100)
    after_pan = (await metrics(page))["ganttFirstDay"]
    add_result(
        results,
        name,
        "gantt-time-axis-drags-fixed-window",
        after_pan != before_pan,
        {"before": before_pan, "after": after_pan},
    )
    add_result(
        results,
        name,
        "gantt-time-axis-drags-by-minute-not-day",
        after_pan != before_pan and not after_pan.endswith("T00:00:00.000Z"),
        {"before": before_pan, "after": after_pan},
    )
    if after_pan != before_pan:
        scale_box = await page.locator(".plan-timeline-scale").bounding_box()
        if scale_box:
            await page.mouse.move(end_x, scale_y)
            await page.mouse.down()
            await page.mouse.move(start_x, scale_y, steps=8)
            await page.mouse.up()
            await page.wait_for_timeout(100)
    before_edge_day = (await metrics(page))["ganttFirstDay"]
    edge_source = page.locator(".plan-timeline-bar").filter(has_text="整理发布检查清单").first
    await edge_source.scroll_into_view_if_needed()
    edge_box = await edge_source.bounding_box()
    if edge_box:
        await page.mouse.move(edge_box["x"] + edge_box["width"] / 2, edge_box["y"] + edge_box["height"] / 2)
        await page.mouse.down()
        await page.mouse.move(width - 4, edge_box["y"] + edge_box["height"] / 2, steps=14)
        await page.wait_for_timeout(180)
        edge_day = (await metrics(page))["ganttFirstDay"]
        await page.mouse.up()
    else:
        edge_day = before_edge_day
    add_result(
        results,
        name,
        "gantt-ticket-drag-at-edge-moves-window",
        edge_day != before_edge_day,
        {"before": before_edge_day, "after": edge_day},
    )
    await page.goto(f"{GUI_URL}?e2eFixture=plan-sessions")
    await page.wait_for_load_state("domcontentloaded")
    await page.wait_for_timeout(250)
    if await page.locator(".plan-conversation-panel .inspector-close").count() > 0:
        await close_plan_panel(page)
    await page.get_by_role("button", name="日历", exact=True).click(force=True)
    await page.wait_for_timeout(100)
    calendar = await metrics(page)
    calendar_header_scroll = await page.evaluate(
        """
        () => {
          const title = document.querySelector('.plan-calendar-title');
          const grid = document.querySelector('.plan-calendar-grid');
          if (!title || !grid) return null;
          const before = title.getBoundingClientRect().left;
          grid.scrollLeft = 96;
          const after = title.getBoundingClientRect().left;
          const buttons = Array.from(
            document.querySelectorAll('.plan-calendar-title .icon-action, .plan-calendar-view-toggle button')
          ).map((button) => {
            const box = button.getBoundingClientRect();
            return { width: box.width, height: box.height };
          });
          return {
            before,
            after,
            delta: Math.abs(before - after),
            gridScrollLeft: grid.scrollLeft,
            canScroll: grid.scrollWidth > grid.clientWidth + 1,
            buttons,
          };
        }
        """
    )
    add_result(
        results,
        name,
        "calendar-mode-renders-month-page",
        calendar["calendarRows"] == 42
        and calendar["calendarEvents"] == 2
        and calendar["calendarViewButtons"] == 3
        and calendar["calendarNavButtons"] == 2
        and calendar["calendarMonthSelected"],
        calendar,
    )
    add_result(
        results,
        name,
        "calendar-header-stays-fixed-while-grid-scrolls",
        bool(
            calendar_header_scroll
            and (
                calendar_header_scroll["gridScrollLeft"] > 0
                or not calendar_header_scroll["canScroll"]
            )
            and calendar_header_scroll["delta"] <= 1
        ),
        calendar_header_scroll,
    )
    add_result(
        results,
        name,
        "calendar-header-hides-count-and-uses-uniform-buttons",
        "4" not in calendar["calendarHeaderText"]
        and calendar_header_scroll
        and all(
            abs(button["width"] - 34) <= 1 and abs(button["height"] - 34) <= 1
            for button in calendar_header_scroll["buttons"]
        ),
        {"text": calendar["calendarHeaderText"], "buttons": calendar_header_scroll["buttons"] if calendar_header_scroll else []},
    )
    add_result(
        results,
        name,
        "calendar-renders-only-timed-scheduled-or-polling",
        calendar["calendarTimedEvents"] == calendar["calendarEvents"]
        and calendar["calendarPollingEvents"] >= 1
        and await page.locator(".plan-calendar").filter(has_text="等待用户补充权限").count() == 0
        and await page.locator(".plan-calendar").filter(has_text="完成 gateway 字段回传").count() == 0
        and await page.locator(".plan-calendar").filter(has_text="用户操作不显示在日历").count() == 0,
        calendar,
    )
    calendar_backgrounds = await ticket_backgrounds(page, ".plan-calendar-event")
    add_result(
        results,
        name,
        "calendar-tickets-do-not-add-status-colors",
        len(set(calendar_backgrounds)) <= 1,
        calendar_backgrounds,
    )
    add_result(
        results,
        name,
        "calendar-tickets-have-subtle-shadow-and-default-cursor",
        calendar["calendarEventShadows"]
        and all("rgba(0, 0, 0, 0.08)" in shadow or "rgb(0 0 0 / 0.08)" in shadow for shadow in calendar["calendarEventShadows"])
        and all(cursor == "default" for cursor in calendar["calendarEventCursors"]),
        {"shadows": calendar["calendarEventShadows"], "cursors": calendar["calendarEventCursors"]},
    )
    add_result(
        results,
        name,
        "calendar-month-events-align-under-uniform-date-headers",
        calendar["calendarEventTops"]
        and len(set(calendar["calendarEventTops"])) == 1
        and calendar["calendarHeaderHeights"]
        and len(set(calendar["calendarHeaderHeights"])) == 1,
        {
            "eventTops": calendar["calendarEventTops"],
            "headerHeights": calendar["calendarHeaderHeights"],
        },
    )
    title_before_next = calendar["calendarTitle"]
    await page.locator(".plan-calendar-nav .icon-action").nth(1).click()
    await page.wait_for_timeout(100)
    title_after_next = (await metrics(page))["calendarTitle"]
    await page.locator(".plan-calendar-nav .icon-action").nth(0).click()
    await page.wait_for_timeout(100)
    title_after_previous = (await metrics(page))["calendarTitle"]
    add_result(
        results,
        name,
        "calendar-month-arrows-page-by-month",
        title_after_next != title_before_next and title_after_previous == title_before_next,
        {
            "before": title_before_next,
            "next": title_after_next,
            "previous": title_after_previous,
        },
    )
    await page.evaluate(
        """
        () => {
          const target = Array.from(document.querySelectorAll('.plan-calendar-cell'))
            .find((cell) => !cell.classList.contains('muted') && !cell.querySelector('.plan-calendar-event'));
          if (!target) throw new Error('empty calendar day not found');
          target.dispatchEvent(new MouseEvent('click', { bubbles: true }));
        }
        """
    )
    await page.wait_for_timeout(150)
    week_metrics = await metrics(page)
    add_result(
        results,
        name,
        "calendar-empty-day-opens-week-view",
        week_metrics["calendarWeekRows"] == 168
        and week_metrics["calendarWeekSelected"]
        and week_metrics["calendarSelectedWeekDay"],
        week_metrics,
    )
    week_title_before_next = week_metrics["calendarTitle"]
    await page.locator(".plan-calendar-nav .icon-action").nth(1).click()
    await page.wait_for_timeout(100)
    week_title_after_next = (await metrics(page))["calendarTitle"]
    await page.locator(".plan-calendar-nav .icon-action").nth(0).click()
    await page.wait_for_timeout(100)
    week_title_after_previous = (await metrics(page))["calendarTitle"]
    add_result(
        results,
        name,
        "calendar-week-arrows-page-by-week",
        week_title_after_next != week_title_before_next
        and week_title_after_previous == week_title_before_next,
        {
            "before": week_title_before_next,
            "next": week_title_after_next,
            "previous": week_title_after_previous,
        },
    )
    empty_week_cell = page.locator(".plan-calendar-hour-cell").filter(has_not=page.locator(".plan-calendar-event")).first
    empty_week_box = await empty_week_cell.bounding_box()
    if empty_week_box:
        await page.mouse.click(empty_week_box["x"] + empty_week_box["width"] * 0.52, empty_week_box["y"] + empty_week_box["height"] * 0.62)
    await page.wait_for_timeout(150)
    draft_from_week = await metrics(page)
    add_result(
        results,
        name,
        "calendar-week-blank-click-opens-new-ticket",
        draft_from_week["splitOpen"]
        and draft_from_week["planControls"]
        and "新工单" in draft_from_week["panelTitle"],
        draft_from_week,
    )
    await page.locator(".plan-conversation-panel .plan-trigger-control:not(.agent-trigger-control) .plan-trigger-button").click()
    await page.locator(".plan-conversation-panel .plan-trigger-option").filter(has_text="定时任务").click()
    await expect(page.locator(".plan-schedule-dialog")).to_be_visible()
    start_date = await page.locator(".plan-schedule-dialog input[type='date']").input_value()
    start_time = await page.locator(".plan-schedule-dialog input[type='time']").input_value()
    add_result(
        results,
        name,
        "calendar-week-click-prefills-scheduled-time",
        len(start_date) == 10 and len(start_time) >= 4,
        {"date": start_date, "time": start_time},
    )
    await page.locator(".plan-schedule-dialog .primary").click()
    add_result(
        results,
        name,
        "draft-session-picker-visible",
        draft_from_week["draftSessionPicker"],
    )
    await page.locator(".plan-conversation-panel .plan-session-button").click()
    session_menu = await metrics(page)
    add_result(
        results,
        name,
        "draft-session-menu-has-new-and-existing-session",
        session_menu["draftSessionMenu"]
        and any("新会话" in row for row in session_menu["draftSessionRows"])
        and any("整理发布检查清单" in row for row in session_menu["draftSessionRows"]),
        session_menu["draftSessionRows"],
    )
    add_result(
        results,
        name,
        "draft-session-menu-session-labels-have-readable-width",
        any(width >= 160 for width in session_menu["draftSessionRowWidths"]),
        session_menu["draftSessionRowWidths"],
    )
    await page.locator(".plan-conversation-panel .plan-session-menu .workspace-pick-row").filter(has_text="整理发布检查清单").click()
    selected_session_label = await page.locator(".plan-conversation-panel .plan-session-button").get_attribute("title")
    add_result(
        results,
        name,
        "draft-session-menu-selects-existing-session",
        selected_session_label and "整理发布检查清单" in selected_session_label,
        selected_session_label,
    )
    await page.locator(".plan-conversation-panel .plan-session-button").click()
    await page.locator(".plan-conversation-panel .plan-session-menu .workspace-pick-row").filter(has_text="新会话").click()
    await page.locator(".plan-conversation-panel .bottom-composer textarea").fill("周视图空白新建工单")
    await page.locator(".plan-conversation-panel .composer-send").click()
    await page.wait_for_timeout(250)
    created_from_calendar = await page.locator(".board-card, .plan-calendar-event").filter(has_text="周视图空白新建工单").count()
    add_result(
        results,
        name,
        "calendar-week-blank-can-create-new-session-ticket",
        created_from_calendar >= 1,
        {"matches": created_from_calendar},
    )
    await close_plan_panel(page)
    await page.get_by_role("button", name="日历", exact=True).click(force=True)
    await page.locator(".plan-calendar-view-toggle button").filter(has_text="月").click()
    await page.wait_for_timeout(100)
    calendar_small_move = await pointer_small_move_state(
        page,
        page.locator(".plan-calendar-event").filter(has_text="整理发布检查清单").first,
    )
    add_result(
        results,
        name,
        "calendar-small-pointer-move-does-not-start-drag",
        calendar_small_move["ghostCount"] == 0
        and calendar_small_move["sourceDraggingCount"] == 0,
        calendar_small_move,
    )
    await close_plan_panel(page)
    month_drag_ghost = await pointer_drag_by_offset(
        page,
        page.locator(".plan-calendar-event").filter(has_text="整理发布检查清单").first,
    )
    add_result(
        results,
        name,
        "calendar-drag-moves-ticket-itself-under-pointer",
        bool(
            month_drag_ghost
            and month_drag_ghost["pointerInside"]
            and month_drag_ghost["topLeftDelta"] <= 2
            and month_drag_ghost["sameWidth"]
            and month_drag_ghost["sameHeight"]
            and "plan-calendar-event" in month_drag_ghost["ghostClass"]
        ),
        month_drag_ghost,
    )
    add_result(
        results,
        name,
        "calendar-drag-hides-source-and-keeps-default-cursor",
        bool(month_drag_ghost and month_drag_ghost["sourceHidden"] and month_drag_ghost["sourceCursor"] == "default"),
        month_drag_ghost,
    )
    await pointer_drag_between(
        page,
        page.locator(".plan-calendar-event").filter(has_text="整理发布检查清单").first,
        page.locator(".plan-calendar-cell").nth(20),
        x_ratio=0.72,
        y_ratio=0.68,
    )
    await page.wait_for_timeout(150)
    add_result(
        results,
        name,
        "calendar-ticket-drags-to-date",
        await page.locator(".plan-calendar-event").filter(has_text="整理发布检查清单").count() == 1,
    )
    await page.locator(".plan-calendar-event").filter(has_text="整理发布检查清单").first.click()
    await expect(page.locator(".plan-conversation-panel")).to_be_visible()
    ticket_open_metrics = await metrics(page)
    add_result(
        results,
        name,
        "calendar-ticket-click-opens-ticket-panel",
        ticket_open_metrics["splitOpen"] and "整理发布检查清单" in ticket_open_metrics["panelTitle"],
    )
    await close_plan_panel(page)
    await page.get_by_role("button", name="日历", exact=True).click(force=True)
    await page.wait_for_timeout(100)
    await page.evaluate(
        """
        () => {
          const cell = Array.from(document.querySelectorAll('.plan-calendar-cell'))
            .find((item) => item.innerText.includes('整理发布检查清单'));
          if (!cell) throw new Error('calendar ticket cell not found');
          cell.dispatchEvent(new MouseEvent('click', { bubbles: true }));
        }
        """
    )
    await page.wait_for_timeout(100)
    await pointer_drag_between(
        page,
        page.locator(".plan-calendar-event").filter(has_text="整理发布检查清单").first,
        page.locator(".plan-calendar-hour-cell").nth(18),
        x_ratio=0.68,
        y_ratio=0.42,
    )
    await page.wait_for_timeout(150)
    add_result(
        results,
        name,
        "calendar-week-ticket-drags-to-hour",
        await page.locator(".plan-calendar-event").filter(has_text="整理发布检查清单").count() == 1,
    )
    source_event = page.locator(".plan-calendar-event").filter(has_text="整理发布检查清单").first
    hour_cells = page.locator(".plan-calendar-hour-cell")
    hour_count = await hour_cells.count()
    target_hour = hour_cells.nth(41 if hour_count > 41 else max(0, hour_count - 1))
    await pointer_drag_between(page, source_event, target_hour, x_ratio=0.72, y_ratio=0.68)
    await page.wait_for_timeout(150)
    precise_text = await page.locator(".plan-calendar-event").filter(has_text="整理发布检查清单").first.inner_text()
    add_result(
        results,
        name,
        "calendar-pointer-drag-keeps-ticket-and-uses-minute-precision",
        "整理发布检查清单" in precise_text and not precise_text.endswith(":00"),
        precise_text,
    )
    await page.locator(".plan-calendar-view-toggle button").filter(has_text="月").click()
    await page.wait_for_timeout(100)
    await page.get_by_role("button", name="待办列表", exact=True).click(force=True)
    await page.wait_for_timeout(100)

    if first["railVisible"]:
        await page.evaluate(
            """
            () => {
              const row = Array.from(document.querySelectorAll('.archived-workspace-node > .workspace-row'))
                .find((item) => item.textContent.includes('tura') || item.title.includes('tura'));
              if (!row) throw new Error('archived workspace row not found');
              row.click();
            }
            """
        )
        archived = await metrics(page)
        add_result(
            results,
            name,
            "archived-group-shows-hidden-session",
            any("隐藏旧会话工单" in row for row in archived["archivedRows"]),
        )
        await page.evaluate(
            """
            () => {
              const card = Array.from(document.querySelectorAll('.board-card'))
                .find((item) => item.innerText.includes('用户操作不显示在日历'));
              const target = Array.from(document.querySelectorAll('.archived-workspace-node > .workspace-row'))
                .find((item) => item.innerText.includes('tura') || item.title.includes('tura'));
              if (!card || !target) throw new Error('left archive drag target not found');
              const dataTransfer = new DataTransfer();
              card.dispatchEvent(new DragEvent('dragstart', { bubbles: true, dataTransfer }));
              target.dispatchEvent(new DragEvent('dragover', { bubbles: true, dataTransfer }));
              target.dispatchEvent(new DragEvent('drop', { bubbles: true, dataTransfer }));
              card.dispatchEvent(new DragEvent('dragend', { bubbles: true, dataTransfer }));
            }
            """
        )
        await page.wait_for_timeout(250)
        archived_after_left_drop = await metrics(page)
        add_result(
            results,
            name,
            "left-rail-archive-drop-updates-session-status",
            any("用户操作不显示在日历" in row for row in archived_after_left_drop["archivedRows"]),
            archived_after_left_drop["archivedRows"],
        )

    await page.locator(".board-card").filter(has_text="整理发布检查清单").first.click(force=True)
    await expect(page.locator(".plan-conversation-panel")).to_be_visible()
    split = await metrics(page)
    add_result(results, name, "ticket-opens-split-conversation", split["splitOpen"])
    add_result(results, name, "split-panel-has-close", split["panelClose"])
    add_result(results, name, "split-panel-has-resize-edge", split["resizeEdge"])
    add_result(
        results,
        name,
        "split-panel-uses-small-ticket-title",
        "整理发布检查清单" in split["panelTitle"],
    )
    add_result(results, name, "ticket-panel-status-editor-visible", split["ticketPanel"])
    add_result(results, name, "split-panel-usable-width", panel_is_usable(split, width), split)
    add_result(results, name, "split-main-keeps-mobile-min-width", enough_main_width(split, width), split)
    add_result(
        results,
        name,
        "split-header-controls-remain-visible",
        split["modeActions"]
        and split["panel"]
        and split["main"]
        and (
            split["panel"]["x"] >= split["main"]["right"]
            or split["modeActions"]["bottom"] <= split["panel"]["y"] + 2
        ),
        split,
    )
    await page.screenshot(path=str(OUT / f"{name}-split.png"), full_page=True)

    await page.get_by_role("button", name="甘特图", exact=True).click(force=True)
    await page.wait_for_timeout(100)
    split_gantt = await metrics(page)
    add_result(
        results,
        name,
        "split-stays-open-while-gantt-renders",
        split_gantt["splitOpen"] and split_gantt["ganttRows"] >= 1,
    )
    await page.get_by_role("button", name="日历", exact=True).click(force=True)
    await page.wait_for_timeout(100)
    split_calendar = await metrics(page)
    add_result(
        results,
        name,
        "split-stays-open-while-calendar-renders",
        split_calendar["splitOpen"]
        and (split_calendar["calendarRows"] == 42 or split_calendar["calendarWeekRows"] == 168)
        and split_calendar["calendarEvents"] >= 1,
        split_calendar,
    )
    await page.get_by_role("button", name="待办列表", exact=True).click(force=True)
    await page.wait_for_timeout(100)

    resize_closed = await drag_resize_to_close(page, width)
    add_result(results, name, "dragging-panel-to-right-closes-it", resize_closed)
    await page.locator(".board-card").filter(has_text="整理发布检查清单").first.click(force=True)
    await expect(page.locator(".plan-conversation-panel")).to_be_visible()

    add_result(
        results,
        name,
        "composer-does-not-show-task-status",
        await page.locator(".plan-composer-status, .plan-option").count() == 0,
    )

    composer = page.locator(".plan-conversation-panel .bottom-composer").first
    if await page.locator(".tool-inspector.open .inspector-close").count() > 0:
        await page.locator(".tool-inspector.open .inspector-close").first.click(force=True)
        await page.wait_for_timeout(150)
    textarea = composer.locator("textarea")
    await textarea.fill("第一处图片在这里\n")
    await composer.locator(".composer-file-input").set_input_files(str(image_path))
    await expect(composer.locator(".composer-image-token")).to_have_count(1)
    await textarea.fill(await textarea.input_value() + "\n图片后面的文字")
    inline_value = await textarea.input_value()
    thumb_box = await composer.locator(".composer-image-token img").first.bounding_box()
    add_result(
        results,
        name,
        "inline-image-token-in-text-position",
        "[[image:" in inline_value and "图片后面的文字" in inline_value,
    )
    add_result(
        results,
        name,
        "inline-image-small-thumbnail",
        bool(thumb_box and thumb_box["width"] <= 50 and thumb_box["height"] <= 40),
        thumb_box,
    )
    await composer.locator(".composer-image-token button").first.click(force=True)
    await expect(page.locator(".media-lightbox")).to_be_visible()
    add_result(
        results,
        name,
        "inline-image-click-opens-browser",
        await page.locator(".media-lightbox img").count() == 1,
    )
    await page.locator(".media-window-actions button").last.click(force=True)
    await composer.locator(".composer-image-token button").filter(has_text="×").first.click(force=True)
    await expect(composer.locator(".composer-image-token")).to_have_count(0)
    add_result(
        results,
        name,
        "inline-image-delete-removes-token",
        "[[image:" not in await textarea.input_value(),
    )
    await composer.locator(".composer-file-input").set_input_files(str(image_path))
    await expect(composer.locator(".composer-image-token")).to_have_count(1)
    add_result(
        results,
        name,
        "inline-image-can-be-added-again",
        "[[image:" in await textarea.input_value(),
    )
    await composer.locator(".composer-file-input").set_input_files(str(attachment_path))
    await expect(composer.locator(".composer-file-token")).to_have_count(1)
    attachment_value = await textarea.input_value()
    add_result(
        results,
        name,
        "inline-file-token-in-text-position",
        "[[file:" in attachment_value
        and attachment_value.index("[[image:") < attachment_value.index("[[file:"),
        attachment_value,
    )
    file_chip = composer.locator(".composer-file-token").first
    file_box = await file_chip.bounding_box()
    add_result(
        results,
        name,
        "inline-file-uses-attachment-chip-style",
        bool(file_box and file_box["height"] <= 34 and file_box["width"] >= 80),
        file_box,
    )
    await file_chip.click(button="right", force=True)
    await expect(page.locator(".composer-attachment-menu")).to_be_visible()
    menu_text = await page.locator(".composer-attachment-menu").inner_text()
    add_result(
        results,
        name,
        "attachment-context-menu-has-file-actions",
        "查看文件" in menu_text and "打开文件位置" in menu_text,
        menu_text,
    )
    await page.mouse.click(5, 5)
    await composer.locator(".composer-file-token button").filter(has_text="×").first.click(force=True)
    await expect(composer.locator(".composer-file-token")).to_have_count(0)
    await textarea.fill("粘贴前\n[[file:missing-token]]\n粘贴后")
    await expect(composer.locator(".composer-rich-editor")).to_contain_text(
        "[[file:missing-token]]"
    )
    add_result(
        results,
        name,
        "unknown-file-token-remains-copyable-text",
        "[[file:missing-token]]" in await textarea.input_value(),
    )

    await close_plan_panel(page)
    await drag_ticket_to_column(page, "完成 gateway 字段回传", "进行中")
    await page.wait_for_timeout(250)
    invalid_done_text = await page.locator(".board-column").filter(has_text="完成").first.inner_text()
    invalid_doing_text = await page.locator(".board-column").filter(has_text="进行中").first.inner_text()
    invalid_panel = await metrics(page)
    add_result(
        results,
        name,
        "done-ticket-cannot-drag-to-doing-and-opens-panel",
        "完成 gateway 字段回传" not in invalid_doing_text
        and invalid_panel["splitOpen"],
        {"done": invalid_done_text, "doing": invalid_doing_text, "panel": invalid_panel["panelTitle"]},
    )
    add_result(
        results,
        name,
        "plan-panel-done-ticket-shows-feedback-prompt",
        any("请输入命令或者反馈" in item for item in invalid_panel["planFeedbackPrompts"]),
        invalid_panel["planFeedbackPrompts"],
    )
    await drag_ticket_to_column(page, "轮询待办工单", "进行中")
    await page.wait_for_timeout(250)
    legal_doing_text = await page.locator(".board-column").filter(has_text="进行中").first.inner_text()
    legal_todo_text = await page.locator(".board-column").filter(has_text="待办").first.inner_text()
    add_result(
        results,
        name,
        "todo-ticket-with-task-can-drag-to-doing",
        "轮询待办工单" in legal_doing_text and "轮询待办工单" not in legal_todo_text,
        {"doing": legal_doing_text, "todo": legal_todo_text},
    )
    await close_plan_panel(page)
    await page.locator(".board-card").filter(has_text="轮询待办工单").first.click(force=True)
    await expect(page.locator(".plan-conversation-panel")).to_be_visible()
    legal_doing_panel = await metrics(page)
    add_result(
        results,
        name,
        "plan-panel-doing-ticket-does-not-show-feedback-prompt",
        not legal_doing_panel["planFeedbackPrompts"],
        legal_doing_panel["planFeedbackPrompts"],
    )
    await close_plan_panel(page)
    await drag_ticket_to_column(page, "整理发布检查清单", "完成")
    await page.wait_for_timeout(250)
    moved_text = await page.locator(".board-column").filter(has_text="完成").first.inner_text()
    todo_text = await page.locator(".board-column").filter(has_text="待办").first.inner_text()
    add_result(results, name, "drag-ticket-to-done", "整理发布检查清单" in moved_text)
    add_result(results, name, "drag-removes-from-todo", "整理发布检查清单" not in todo_text)
    await drag_ticket_to_column(page, "整理发布检查清单", "待反馈")
    await page.wait_for_timeout(250)
    invalid_question_text = await page.locator(".board-column").filter(has_text="待反馈").first.inner_text()
    invalid_question_panel = await metrics(page)
    add_result(
        results,
        name,
        "ticket-cannot-drag-into-question-and-opens-panel",
        "整理发布检查清单" not in invalid_question_text
        and invalid_question_panel["splitOpen"],
        {
            "question": invalid_question_text,
            "panel": invalid_question_panel["panelTitle"],
        },
    )
    await close_plan_panel(page)
    await page.evaluate(
        """
        () => {
          const card = Array.from(document.querySelectorAll('.board-card'))
            .find((item) => item.innerText.includes('轮询待办工单'));
          const target = document.querySelector('.board-archive-zone');
          if (!card || !target) throw new Error('board archive zone not found');
          const dataTransfer = new DataTransfer();
          card.dispatchEvent(new DragEvent('dragstart', { bubbles: true, dataTransfer }));
          target.dispatchEvent(new DragEvent('dragover', { bubbles: true, dataTransfer }));
          target.dispatchEvent(new DragEvent('drop', { bubbles: true, dataTransfer }));
          card.dispatchEvent(new DragEvent('dragend', { bubbles: true, dataTransfer }));
        }
        """
    )
    await page.wait_for_timeout(250)
    archive_drop_metrics = await metrics(page)
    add_result(
        results,
        name,
        "board-right-archive-drop-updates-session-status",
        "轮询待办工单" not in "\n".join(archive_drop_metrics["cards"]),
        archive_drop_metrics["cards"],
    )

    await page.locator(".board-column").filter(has_text="待办").first.locator(".icon-action").click()
    await expect(page.locator(".plan-conversation-panel .plan-trigger-control:not(.agent-trigger-control)")).to_be_visible()
    draft_open = await metrics(page)
    add_result(results, name, "new-ticket-uses-right-composer", draft_open["planControls"])
    add_result(results, name, "old-draft-ui-deleted", draft_open["removedDraftPanels"] == 0)
    add_result(results, name, "draft-main-keeps-mobile-min-width", enough_main_width(draft_open, width), draft_open)
    await page.locator(".plan-conversation-panel .plan-session-button").click()
    await page.locator(".plan-conversation-panel .session-pick-row").filter(has_text="实现拖拽状态切换").click()
    await page.wait_for_timeout(250)
    inherited_session_metrics = await metrics(page)
    add_result(
        results,
        name,
        "draft-existing-session-selection-loads-session-history",
        "实现拖拽状态切换" in inherited_session_metrics["panelTitle"]
        and await page.locator(".plan-conversation-panel").filter(has_text="用户创建工单：实现拖拽状态切换").count()
        == 1,
        {
            "panel": inherited_session_metrics["panelTitle"],
            "text": await page.locator(".plan-conversation-panel").inner_text(),
        },
    )
    await page.locator(".plan-conversation-panel .plan-session-button").click()
    await page.locator(".plan-conversation-panel .session-pick-row").filter(has_text="新会话").click()
    await page.wait_for_timeout(150)
    await page.locator(".plan-conversation-panel .bottom-composer textarea").fill("需要审批后继续执行")
    await page.locator(".plan-conversation-panel .plan-trigger-control:not(.agent-trigger-control) .plan-trigger-button").click()
    add_result(
        results,
        name,
        "plan-panel-trigger-menu-has-run-now",
        await page.locator(".plan-conversation-panel .plan-trigger-option").filter(has_text="立刻执行").count()
        == 1,
    )
    await page.locator(".plan-conversation-panel .plan-trigger-option").filter(has_text="定时任务").click()
    await expect(page.locator(".plan-schedule-dialog")).to_be_visible()
    await page.locator(".plan-schedule-dialog input[type='date']").fill("2026-05-25")
    await page.locator(".plan-schedule-dialog input[type='time']").fill("10:30")
    await page.locator(".plan-schedule-dialog .primary").click()
    await page.locator(".plan-conversation-panel .plan-trigger-control:not(.agent-trigger-control) .plan-trigger-button").click()
    add_result(
        results,
        name,
        "scheduled-trigger-option-shows-check",
        await page.locator(".plan-conversation-panel .plan-trigger-option.selected svg").count() == 1,
    )
    menu_geometry = await trigger_menu_geometry(page, ".plan-conversation-panel")
    add_result(
        results,
        name,
        "plan-trigger-menu-left-aligned-above-button",
        bool(
            menu_geometry
            and menu_geometry["visible"]
            and menu_geometry["leftDelta"] <= 4
            and -1 <= menu_geometry["gap"] <= 1
            and all(abs(height - 34) <= 1 for height in menu_geometry["toolbarButtonHeights"])
            and all(abs(height - 34) <= 1 for height in menu_geometry["menuOptionHeights"])
            and "plan-trigger" in menu_geometry["topElement"]
            and "plan-trigger" in menu_geometry["bottomElement"]
        ),
        menu_geometry,
    )
    await page.locator(".plan-conversation-panel .plan-trigger-control:not(.agent-trigger-control) .plan-trigger-button").click()
    scheduled_text = await page.locator(".plan-conversation-panel .plan-trigger-control:not(.agent-trigger-control) .plan-trigger-button").inner_text()
    if width <= 640:
        add_result(results, name, "phone-trigger-button-icon-only", (await metrics(page))["triggerIconOnly"])
    else:
        add_result(
            results,
            name,
            "scheduled-trigger-button-hides-time",
            "定时任务" in scheduled_text and "2026" not in scheduled_text,
        )
    add_result(
        results,
        name,
        "scheduled-time-not-inline-in-composer",
        await page.locator(".plan-conversation-panel .plan-trigger-fields input[type='date'], .plan-conversation-panel .plan-trigger-fields input[type='time']").count()
        == 0,
    )
    add_result(
        results,
        name,
        "scheduled-time-not-inside-menu",
        await page.locator(".plan-conversation-panel .plan-trigger-menu input[type='date'], .plan-conversation-panel .plan-trigger-menu input[type='time']").count()
        == 0,
    )
    await page.locator(".plan-conversation-panel .plan-trigger-control:not(.agent-trigger-control) .plan-trigger-button").click()
    await page.locator(".plan-conversation-panel .plan-trigger-option").filter(has_text="轮询任务").click()
    await expect(page.locator(".plan-schedule-dialog .plan-schedule-interval-grid")).to_be_visible()
    add_result(
        results,
        name,
        "schedule-dialog-header-and-interval-inputs-are-clean",
        await page.locator(".plan-schedule-dialog header p").count() == 0
        and await page.locator(".plan-schedule-dialog .plan-schedule-interval-grid input[type='number']").count()
        == 0
        and await page.locator(".plan-schedule-dialog .plan-schedule-interval-grid input[inputmode='numeric']").count()
        == 4,
    )
    schedule_dialog_metrics = await metrics(page)
    add_result(
        results,
        name,
        "schedule-dialog-uses-localized-interval-labels-and-lengths",
        schedule_dialog_metrics["scheduleIntervalLabels"] == ["天", "时", "分", "秒"]
        and schedule_dialog_metrics["scheduleIntervalMaxLengths"] == ["3", "2", "2", "2"],
        {
            "labels": schedule_dialog_metrics["scheduleIntervalLabels"],
            "lengths": schedule_dialog_metrics["scheduleIntervalMaxLengths"],
        },
    )
    add_result(
        results,
        name,
        "schedule-dialog-close-aligns-with-title-row",
        schedule_dialog_metrics["scheduleCloseBox"]
        and schedule_dialog_metrics["scheduleTitleBox"]
        and abs(schedule_dialog_metrics["scheduleCloseBox"]["y"] - schedule_dialog_metrics["scheduleTitleBox"]["y"]) <= 8,
        {
            "close": schedule_dialog_metrics["scheduleCloseBox"],
            "title": schedule_dialog_metrics["scheduleTitleBox"],
        },
    )
    await page.locator(".plan-schedule-dialog .plan-schedule-interval-grid input").nth(0).fill("12ab-3")
    await page.wait_for_timeout(50)
    sanitized_interval_value = await page.evaluate(
        "() => document.querySelector('.plan-schedule-dialog .plan-schedule-interval-grid input')?.value ?? ''"
    )
    add_result(
        results,
        name,
        "schedule-dialog-interval-inputs-strip-invalid-characters",
        sanitized_interval_value.isdigit() and all(ch not in sanitized_interval_value for ch in "ab-"),
        sanitized_interval_value,
    )
    await page.locator(".plan-schedule-dialog input[type='date']").fill("2026-05-25")
    await page.locator(".plan-schedule-dialog input[type='time']").fill("11:45")
    await page.locator(".plan-schedule-dialog .plan-schedule-interval-grid input").nth(0).fill("1")
    await page.locator(".plan-schedule-dialog .plan-schedule-interval-grid input").nth(1).fill("2")
    await page.locator(".plan-schedule-dialog .plan-schedule-interval-grid input").nth(2).fill("30")
    await page.locator(".plan-schedule-dialog .primary").click()
    poll_text = await page.locator(".plan-conversation-panel .plan-trigger-control:not(.agent-trigger-control) .plan-trigger-button").inner_text()
    add_result(
        results,
        name,
        "polling-trigger-uses-schedule-dialog-not-inline-inputs",
        await page.locator(".plan-conversation-panel .plan-trigger-fields input").count() == 0,
    )
    if width > 640:
        add_result(
            results,
            name,
            "polling-trigger-button-hides-time-and-interval",
            "轮询任务" in poll_text and "2026" not in poll_text and "30m" not in poll_text,
        )
    await page.locator(".plan-conversation-panel .plan-trigger-control:not(.agent-trigger-control) .plan-trigger-button").click()
    add_result(
        results,
        name,
        "polling-trigger-option-shows-check",
        await page.locator(".plan-conversation-panel .plan-trigger-option.selected svg").count() == 1,
    )
    await page.locator(".plan-conversation-panel .plan-trigger-control:not(.agent-trigger-control) .plan-trigger-button").click()
    await page.locator(".plan-conversation-panel .composer-send").click()
    try:
        await page.wait_for_function(
            "() => Array.from(document.querySelectorAll('.board-column')).some((column) => column.innerText.includes('待办') && column.innerText.includes('需要审批后继续执行'))",
            timeout=2_000,
        )
    except Exception:
        await page.wait_for_timeout(250)
    created_todo_text = await page.locator(".board-column").filter(has_text="待办").first.inner_text()
    add_result(
        results,
        name,
        "create-ticket-from-composer-in-todo",
        "需要审批后继续执行" in created_todo_text,
    )

    await close_plan_panel(page)
    closed = await metrics(page)
    add_result(results, name, "plan-panel-close-hides-right-side", not closed["splitOpen"])
    add_result(results, name, "final-no-error-strip", closed["error"] == "")
    add_result(results, name, "final-no-document-overflow", closed["overflowX"] <= 1)
    add_result(results, name, "final-main-keeps-mobile-min-width", enough_main_width(closed, width), closed)
    await page.screenshot(path=str(OUT / f"{name}-after-actions.png"), full_page=True)
    await page.close()
    return results


async def main():
    OUT.mkdir(parents=True, exist_ok=True)
    image_path = OUT / "inline-composer.png"
    image_path.write_bytes(
        base64.b64decode(
            "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFElEQVR4nGNkaPj/nwEJMDGgAcQYADNnBAKfgCBtAAAAAElFTkSuQmCC"
        )
    )
    attachment_path = OUT / "session-plan-connection-requirements.md"
    attachment_path.write_text("# session plan\n\nattachment check\n", encoding="utf-8")
    results = []
    browser_errors = []
    async with async_playwright() as playwright:
        browser = await playwright.chromium.launch(headless=True)
        for viewport in VIEWPORTS:
            results.extend(
                await run_plan_flow(
                    browser,
                    viewport,
                    image_path,
                    attachment_path,
                    browser_errors,
                )
            )
        await browser.close()

    ignored = [item for item in browser_errors if "favicon" in item["text"].lower()]
    blocking = [item for item in browser_errors if item not in ignored]
    summary = {
        "viewports": VIEWPORTS,
        "results": results,
        "browserErrors": blocking,
        "ignoredBrowserErrors": ignored,
    }
    (OUT / "summary.json").write_text(
        json.dumps(summary, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    failures = [item for item in results if not item["ok"]]
    if blocking:
        failures.append({"name": "browser-errors", "detail": blocking})
    if failures:
        raise SystemExit(json.dumps(failures, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    asyncio.run(main())
