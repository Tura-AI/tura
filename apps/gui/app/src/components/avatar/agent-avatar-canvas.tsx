import type { AgentAvatarConfig, PersonaMediaConfig } from "@tura/gateway-sdk";
import { createEffect, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import { classNames } from "../../state/format";
import { EMOJI_ALIASES, avatarExpressionIdsForEmoji } from "./agent-avatar-protocol";
import {
  FALLBACK_AVATAR_IMAGE,
  avatarImageKey,
  avatarImageKeyForLoaded,
  avatarPixelAfterThreshold,
  fallbackImageKey,
  type AvatarExpressionInfo,
} from "./agent-avatar-rendering";

export type AvatarRenderSettings = AgentAvatarConfig;
export type AvatarDisplayMode = NonNullable<AgentAvatarConfig["display_mode"]>;
export const AVATAR_WORKSPACE_CONFIG_KEY = "agent_avatar";

export const DEFAULT_AVATAR_SETTINGS: AvatarRenderSettings = {
  role: "tura",
  display_mode: "static",
  pixel_size: 20,
  threshold: 160,
};

export const AVATAR_SETTING_LIMITS = {
  pixelSize: { min: 10, max: 30 },
  threshold: { min: 100, max: 200 },
};

const CANVAS_SIZE = 768;
const POINTER_DIRECTION_DELAY_MS = 50;
const DIRECTIONS = [
  "center",
  "up",
  "down",
  "left",
  "right",
  "up-left",
  "up-right",
  "down-left",
  "down-right",
];
const RADIAL_DIRECTIONS = [
  "right",
  "down-right",
  "down",
  "down-left",
  "left",
  "up-left",
  "up",
  "up-right",
];
const FALLBACK_EXPRESSIONS = [
  "panic",
  "crying",
  "confused",
  "nervous",
  "vigilant",
  "laugh",
  "smirk",
  "tired",
];
type LoadedImages = Record<string, HTMLImageElement>;
type ImageEntry = {
  key: string;
  src: string;
};

const avatarImageCache = new Map<string, HTMLImageElement>();
const avatarImageRequestCache = new Map<string, Promise<HTMLImageElement | undefined>>();

function fallbackMedia(role: string): PersonaMediaConfig {
  return {
    name: role,
    root_directory: `/assets/persona/${role}/media`,
    expression_directory: `/assets/persona/${role}/media/expressions`,
    direction_order: DIRECTIONS,
    default_expression: "vigilant",
    default_direction: "right",
    expressions: FALLBACK_EXPRESSIONS.map((id) => ({
      id,
      name: id,
      emoji_aliases: EMOJI_ALIASES[id] ?? [],
      source_directory: `/assets/persona/${role}/media/expressions/${id}`,
      grid_path: `/assets/persona/${role}/media/expressions/${id}/grid/sheet.png`,
      frames: Object.fromEntries(
        DIRECTIONS.map((direction) => [
          direction,
          `/assets/persona/${role}/media/expressions/${id}/frames/${direction}.png`,
        ]),
      ),
    })),
  };
}

export function normalizeAvatarSettings(
  value?: Partial<AvatarRenderSettings> | null,
): AvatarRenderSettings {
  return {
    role: value?.role || DEFAULT_AVATAR_SETTINGS.role,
    persona_id: value?.persona_id,
    display_mode: normalizeAvatarDisplayMode(value?.display_mode),
    pixel_size: clamp(
      Number(value?.pixel_size ?? DEFAULT_AVATAR_SETTINGS.pixel_size),
      AVATAR_SETTING_LIMITS.pixelSize.min,
      AVATAR_SETTING_LIMITS.pixelSize.max,
    ),
    threshold: clamp(
      Number(value?.threshold ?? DEFAULT_AVATAR_SETTINGS.threshold),
      AVATAR_SETTING_LIMITS.threshold.min,
      AVATAR_SETTING_LIMITS.threshold.max,
    ),
  };
}

export function normalizeAvatarDisplayMode(value: unknown): AvatarDisplayMode {
  return value === "hidden" || value === "dynamic" ? value : "static";
}

export function avatarSettingsFromConfigValue(value: unknown): AvatarRenderSettings {
  if (!value) {
    return normalizeAvatarSettings({
      ...DEFAULT_AVATAR_SETTINGS,
      persona_id: DEFAULT_AVATAR_SETTINGS.role,
    });
  }
  if (typeof value === "string") {
    try {
      return avatarSettingsFromConfigValue(JSON.parse(value));
    } catch {
      return normalizeAvatarSettings({
        ...DEFAULT_AVATAR_SETTINGS,
        persona_id: DEFAULT_AVATAR_SETTINGS.role,
      });
    }
  }
  if (typeof value !== "object" || Array.isArray(value)) {
    return normalizeAvatarSettings({
      ...DEFAULT_AVATAR_SETTINGS,
      persona_id: DEFAULT_AVATAR_SETTINGS.role,
    });
  }
  const settings = normalizeAvatarSettings(value as Partial<AvatarRenderSettings>);
  return {
    ...settings,
    persona_id: settings.persona_id ?? settings.role,
  };
}

export function agentAvatarMedia(
  media: PersonaMediaConfig | null | undefined,
  role: string | undefined,
): PersonaMediaConfig {
  return media ?? fallbackMedia(role || DEFAULT_AVATAR_SETTINGS.role || "tura");
}

export function AgentAvatarCanvas(props: {
  media?: PersonaMediaConfig | null;
  settings?: Partial<AvatarRenderSettings> | null;
  expressionEmoji?: string;
  expressionId?: string;
  interactive?: boolean;
  previewCycle?: boolean;
  class?: string;
  label?: string;
}) {
  const settings = createMemo(() => normalizeAvatarSettings(props.settings));
  const media = createMemo(() => agentAvatarMedia(props.media, settings().role));
  const expressions = createMemo(() => expressionInfos(media()));
  const [images, setImages] = createSignal<LoadedImages>({});
  const [loading, setLoading] = createSignal(true);
  const [direction, setDirection] = createSignal("right");
  const [expression, setExpression] = createSignal(
    media().default_expression || expressions()[0]?.id || "vigilant",
  );
  let canvas: HTMLCanvasElement | undefined;
  let drawFrame: number | undefined;
  let previewTimer: number | undefined;
  let pointerDirectionTimer: number | undefined;
  let loadRequestId = 0;
  let lastDirectionChangeAt = 0;
  const offscreen = document.createElement("canvas");
  const offscreenContext = offscreen.getContext("2d", {
    willReadFrequently: true,
  });

  function queueDraw() {
    if (drawFrame) {
      cancelAnimationFrame(drawFrame);
    }
    drawFrame = requestAnimationFrame(() => {
      drawFrame = undefined;
      drawAvatar();
    });
  }

  function chooseExpressionForEmoji(emoji: string | undefined): string | undefined {
    return randomItem(avatarExpressionIdsForEmoji(media(), emoji));
  }

  createEffect(() => {
    const nextMedia = media();
    const nextExpressions = expressions();
    setExpression(
      props.expressionId || nextMedia.default_expression || nextExpressions[0]?.id || "vigilant",
    );
    setDirection("right");
    const requestId = ++loadRequestId;
    setLoading(!allImagesCached(nextExpressions));
    void loadImages(nextExpressions).then((loaded) => {
      if (requestId !== loadRequestId) {
        return;
      }
      setImages(loaded);
      setLoading(false);
    });
  });

  createEffect(() => {
    if (props.expressionId) {
      setExpression(props.expressionId);
      return;
    }
    const next = chooseExpressionForEmoji(props.expressionEmoji);
    if (next) {
      setExpression(next);
    }
  });

  createEffect(() => {
    images();
    direction();
    expression();
    settings();
    queueDraw();
  });

  createEffect(() => {
    if (previewTimer) {
      window.clearInterval(previewTimer);
      previewTimer = undefined;
    }
    if (!props.previewCycle || props.expressionId) {
      return;
    }
    previewTimer = window.setInterval(() => {
      const next = randomItem(expressions());
      if (next) {
        setExpression(next.id);
      }
    }, 5000);
  });

  createEffect(() => {
    if (props.interactive === false) {
      setDirection("right");
    }
  });

  onMount(() => {
    const commitDirection = (nextDirection: string) => {
      const currentDirection = direction();
      if (nextDirection === currentDirection) {
        return;
      }
      const now = performance.now();
      const elapsed = now - lastDirectionChangeAt;
      const shouldHold =
        elapsed < POINTER_DIRECTION_DELAY_MS &&
        !directionsAdjacent(currentDirection, nextDirection);
      if (pointerDirectionTimer) {
        window.clearTimeout(pointerDirectionTimer);
        pointerDirectionTimer = undefined;
      }
      if (!shouldHold) {
        lastDirectionChangeAt = now;
        setDirection(nextDirection);
        return;
      }
      pointerDirectionTimer = window.setTimeout(
        () => {
          pointerDirectionTimer = undefined;
          lastDirectionChangeAt = performance.now();
          setDirection(nextDirection);
        },
        Math.max(0, POINTER_DIRECTION_DELAY_MS - elapsed),
      );
    };
    const updateDirection = (clientX: number, clientY: number) => {
      if (!canvas || props.interactive === false) {
        return;
      }
      commitDirection(directionFromPointer(canvas, clientX, clientY));
    };
    const pointerMove = (event: PointerEvent) => updateDirection(event.clientX, event.clientY);
    const pointerDown = (event: PointerEvent) => updateDirection(event.clientX, event.clientY);
    const mouseMove = (event: MouseEvent) => updateDirection(event.clientX, event.clientY);
    const mouseDown = (event: MouseEvent) => updateDirection(event.clientX, event.clientY);
    const touchStart = (event: TouchEvent) => {
      const touch = event.touches[0] ?? event.changedTouches[0];
      if (touch) {
        updateDirection(touch.clientX, touch.clientY);
      }
    };
    const themeObserver = new MutationObserver(queueDraw);
    window.addEventListener("pointermove", pointerMove);
    window.addEventListener("pointerdown", pointerDown);
    window.addEventListener("mousemove", mouseMove);
    window.addEventListener("mousedown", mouseDown);
    window.addEventListener("touchstart", touchStart, { passive: true });
    document.addEventListener("pointermove", pointerMove, true);
    document.addEventListener("pointerdown", pointerDown, true);
    document.addEventListener("mousemove", mouseMove, true);
    document.addEventListener("mousedown", mouseDown, true);
    document.addEventListener("touchstart", touchStart, {
      capture: true,
      passive: true,
    });
    themeObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme", "style"],
    });
    onCleanup(() => {
      window.removeEventListener("pointermove", pointerMove);
      window.removeEventListener("pointerdown", pointerDown);
      window.removeEventListener("mousemove", mouseMove);
      window.removeEventListener("mousedown", mouseDown);
      window.removeEventListener("touchstart", touchStart);
      document.removeEventListener("pointermove", pointerMove, true);
      document.removeEventListener("pointerdown", pointerDown, true);
      document.removeEventListener("mousemove", mouseMove, true);
      document.removeEventListener("mousedown", mouseDown, true);
      document.removeEventListener("touchstart", touchStart, true);
      themeObserver.disconnect();
      if (previewTimer) {
        window.clearInterval(previewTimer);
      }
      if (pointerDirectionTimer) {
        window.clearTimeout(pointerDirectionTimer);
      }
      if (drawFrame) {
        cancelAnimationFrame(drawFrame);
      }
    });
  });

  function drawAvatar() {
    if (!canvas || !offscreenContext) {
      return;
    }
    const context = canvas.getContext("2d");
    if (!context) {
      return;
    }
    const image =
      images()[
        avatarImageKeyForLoaded(
          expressions(),
          Object.keys(images()),
          expression(),
          direction(),
          media().default_direction || "right",
          media().default_expression,
        )
      ];
    if (!image) {
      return;
    }

    context.clearRect(0, 0, CANVAS_SIZE, CANVAS_SIZE);
    const pixelSize = settings().pixel_size;
    const identity = pixelSize <= 0;
    const smallWidth = identity ? CANVAS_SIZE : Math.max(1, Math.floor(CANVAS_SIZE / pixelSize));
    const smallHeight = identity ? CANVAS_SIZE : Math.max(1, Math.floor(CANVAS_SIZE / pixelSize));
    offscreen.width = smallWidth;
    offscreen.height = smallHeight;
    offscreenContext.imageSmoothingEnabled = true;
    offscreenContext.clearRect(0, 0, smallWidth, smallHeight);
    offscreenContext.drawImage(image, 0, 0, smallWidth, smallHeight);
    applyBlackWhiteTransparency(offscreenContext, smallWidth, smallHeight);

    const baseWidth = identity ? CANVAS_SIZE : smallWidth * pixelSize;
    const baseHeight = identity ? CANVAS_SIZE : smallHeight * pixelSize;
    const drawWidth = Math.max(1, Math.round(baseWidth));
    const drawHeight = Math.max(1, Math.round(baseHeight));
    const offsetX = Math.round((CANVAS_SIZE - drawWidth) / 2);
    const offsetY = Math.round((CANVAS_SIZE - drawHeight) / 2);
    context.imageSmoothingEnabled = false;
    context.drawImage(
      offscreen,
      0,
      0,
      smallWidth,
      smallHeight,
      offsetX,
      offsetY,
      drawWidth,
      drawHeight,
    );
  }

  function applyBlackWhiteTransparency(
    context: CanvasRenderingContext2D,
    width: number,
    height: number,
  ) {
    const imageData = context.getImageData(0, 0, width, height);
    const data = imageData.data;
    for (let index = 0; index < data.length; index += 4) {
      const originalAlpha = data[index + 3] ?? 0;
      const darkTheme = isDarkTheme();
      if (!darkTheme && originalAlpha <= 8) {
        data[index + 3] = 0;
        continue;
      }
      const gray =
        (data[index] ?? 0) * 0.299 +
        (data[index + 1] ?? 0) * 0.587 +
        (data[index + 2] ?? 0) * 0.114;
      const pixel = avatarPixelAfterThreshold(
        gray,
        originalAlpha,
        settings().threshold,
        darkTheme,
      );
      data[index] = pixel.value;
      data[index + 1] = pixel.value;
      data[index + 2] = pixel.value;
      data[index + 3] = pixel.alpha;
    }
    context.putImageData(imageData, 0, 0);
  }

  return (
    <div
      class={classNames("agent-avatar-stage", loading() && "loading", props.class)}
      role={props.label ? "img" : undefined}
      aria-label={props.label}
      aria-hidden={props.label ? undefined : "true"}
      data-avatar-direction={direction()}
      data-avatar-expression={expression()}
    >
      <canvas
        ref={canvas}
        class="agent-avatar-canvas"
        width={CANVAS_SIZE}
        height={CANVAS_SIZE}
        aria-hidden="true"
      />
      <div class="agent-avatar-loading" aria-hidden="true">
        <span />
      </div>
    </div>
  );
}

function expressionInfos(media: PersonaMediaConfig): AvatarExpressionInfo[] {
  return (media.expressions ?? []).map((expression) => ({
    id: expression.id,
    aliases: [
      ...(expression.emoji_aliases ?? []),
      ...((expression as unknown as { emojiAliases?: string[] }).emojiAliases ?? []),
      ...(EMOJI_ALIASES[expression.id] ?? []),
    ].filter(Boolean),
    frames: expression.frames,
  }));
}

function imageEntries(expressions: AvatarExpressionInfo[]): ImageEntry[] {
  const entries: ImageEntry[] = expressions.flatMap((expression) =>
    Object.entries(expression.frames).map(([direction, src]) => ({
      key: avatarImageKey(expression.id, direction),
      src,
    })),
  );
  entries.push({ key: fallbackImageKey(), src: FALLBACK_AVATAR_IMAGE });
  return entries;
}

function allImagesCached(expressions: AvatarExpressionInfo[]): boolean {
  return imageEntries(expressions).every((entry) => avatarImageCache.has(mediaSource(entry.src)));
}

function loadImages(expressions: AvatarExpressionInfo[]): Promise<LoadedImages> {
  const entries = imageEntries(expressions);
  return Promise.all(
    entries.map(async ({ key, src }) => [key, await loadCachedImage(mediaSource(src))]),
  ).then((loaded) =>
    Object.fromEntries(
      loaded.filter((item): item is [string, HTMLImageElement] => item[1] !== undefined),
    ),
  );
}

function loadCachedImage(src: string): Promise<HTMLImageElement | undefined> {
  const cached = avatarImageCache.get(src);
  if (cached) {
    return Promise.resolve(cached);
  }
  const pending = avatarImageRequestCache.get(src);
  if (pending) {
    return pending;
  }
  const request = new Promise<HTMLImageElement | undefined>((resolve) => {
    const image = new Image();
    image.onload = () => {
      avatarImageCache.set(src, image);
      avatarImageRequestCache.delete(src);
      resolve(image);
    };
    image.onerror = () => {
      avatarImageRequestCache.delete(src);
      resolve(undefined);
    };
    image.src = src;
  });
  avatarImageRequestCache.set(src, request);
  return request;
}

function mediaSource(path: string): string {
  if (/^(https?:|data:|\/)/iu.test(path)) {
    return path;
  }
  const normalized = path.replace(/\\/gu, "/");
  const personaAsset = normalized.match(
    /(?:^|\/)(?:crates\/persona|personas)\/src\/([^/]+)\/media\/(.+)$/u,
  );
  if (personaAsset) {
    return `/assets/persona/${personaAsset[1]}/media/${personaAsset[2]}`;
  }
  const publicPersonaAsset = normalized.match(/(?:^|\/)assets\/persona\/([^/]+)\/media\/(.+)$/u);
  if (publicPersonaAsset) {
    return `/assets/persona/${publicPersonaAsset[1]}/media/${publicPersonaAsset[2]}`;
  }
  return `/assets/${normalized.replace(/^.*\//u, "")}`;
}

function directionFromPointer(canvas: HTMLCanvasElement, clientX: number, clientY: number): string {
  const rect = canvas.getBoundingClientRect();
  const centerX = rect.left + rect.width / 2;
  const centerY = rect.top + rect.height / 2;
  const dx = clientX - centerX;
  const dy = clientY - centerY;
  if (Math.hypot(dx, dy) < rect.width * 0.12) {
    return "center";
  }
  const angle = Math.atan2(dy, dx) * (180 / Math.PI);
  if (angle >= -22.5 && angle < 22.5) return "right";
  if (angle >= 22.5 && angle < 67.5) return "down-right";
  if (angle >= 67.5 && angle < 112.5) return "down";
  if (angle >= 112.5 && angle < 157.5) return "down-left";
  if (angle >= 157.5 || angle < -157.5) return "left";
  if (angle >= -157.5 && angle < -112.5) return "up-left";
  if (angle >= -112.5 && angle < -67.5) return "up";
  return "up-right";
}

function directionsAdjacent(current: string, next: string): boolean {
  if (current === next || current === "center" || next === "center") {
    return true;
  }
  const currentIndex = RADIAL_DIRECTIONS.indexOf(current);
  const nextIndex = RADIAL_DIRECTIONS.indexOf(next);
  if (currentIndex < 0 || nextIndex < 0) {
    return true;
  }
  const distance = Math.abs(currentIndex - nextIndex);
  return Math.min(distance, RADIAL_DIRECTIONS.length - distance) <= 1;
}

function isDarkTheme(): boolean {
  const paper = getComputedStyle(document.documentElement).getPropertyValue("--paper").trim();
  const rgb = parseCssColor(paper);
  if (!rgb) {
    return ["dark", "liangzhu"].includes(document.documentElement.dataset.theme ?? "");
  }
  const luminance = (0.2126 * rgb.r + 0.7152 * rgb.g + 0.0722 * rgb.b) / 255;
  return luminance < 0.45;
}

function parseCssColor(value: string): { r: number; g: number; b: number } | undefined {
  const hex = value.match(/^#([0-9a-f]{3}|[0-9a-f]{6})$/iu)?.[1];
  if (hex) {
    const full =
      hex.length === 3
        ? hex
            .split("")
            .map((part) => `${part}${part}`)
            .join("")
        : hex;
    return {
      r: Number.parseInt(full.slice(0, 2), 16),
      g: Number.parseInt(full.slice(2, 4), 16),
      b: Number.parseInt(full.slice(4, 6), 16),
    };
  }
  const rgb = value.match(/^rgba?\(\s*([.\d]+)\s*,\s*([.\d]+)\s*,\s*([.\d]+)/iu);
  if (!rgb) {
    return undefined;
  }
  return {
    r: Number(rgb[1]),
    g: Number(rgb[2]),
    b: Number(rgb[3]),
  };
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(Number.isFinite(value) ? value : min, min), max);
}

function randomItem<T>(items: T[]): T | undefined {
  if (items.length === 0) {
    return undefined;
  }
  return items[Math.floor(Math.random() * items.length)];
}
