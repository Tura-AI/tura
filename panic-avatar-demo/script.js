const canvas = document.querySelector("#avatar");
const context = canvas.getContext("2d");
const pixelSizeInput = document.querySelector("#pixelSize");
const pixelValue = document.querySelector("#pixelValue");
const thresholdInput = document.querySelector("#threshold");
const thresholdValue = document.querySelector("#thresholdValue");
const bgToneInput = document.querySelector("#bgTone");
const bgToneValue = document.querySelector("#bgToneValue");
const avatarScaleInput = document.querySelector("#avatarScale");
const avatarScaleValue = document.querySelector("#avatarScaleValue");
const emojiMapSelect = document.querySelector("#emojiMap");
const expressionGrid = document.querySelector("#expressionGrid");
const expressionButtonsContainer = expressionGrid;
const modeButtons = document.querySelectorAll(".mode-button");
const roleButtons = document.querySelectorAll(".role-button");

const directions = [
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

const expressionLabels = {
  panic: "慌张",
  crying: "慌哭",
  confused: "困惑",
  nervous: "强笑",
  vigilant: "警惕",
};

const expressionAliases = {
  panic: ["😱", "😨", "😰", "😵", "🥶"],
  crying: ["😭", "😢", "😿", "😥", "😭\u200d💧"],
  confused: ["😕", "🤔", "😳", "🙄", "😵\u200d💫"],
  nervous: ["😬", "😟", "😓", "😮\u200d💨", "🥴"],
  vigilant: ["👀", "🫢", "🫣", "😮", "😯"],
};

const roleExpressionIds = {
  tura: ["panic", "crying", "confused", "nervous", "vigilant"],
  wonderful: ["vigilant"],
  pidan: ["vigilant"],
};

const expressionSources = {};

Object.entries(roleExpressionIds).forEach(([role, expressionIds]) => {
  expressionSources[role] = {};
  expressionIds.forEach((expression) => {
    const frames = {};
    directions.forEach((direction) => {
      frames[direction] = `./assets/system/${role}/${expression}/frames/${direction}.png`;
    });
    expressionSources[role][expression] = frames;
  });
});

const images = {};
const offscreen = document.createElement("canvas");
const offscreenContext = offscreen.getContext("2d");

let currentRole = "tura";
let currentDirection = "center";
let currentExpression = "panic";
let pixelSize = Number(pixelSizeInput.value);
let threshold = Number(thresholdInput.value);
let bgTone = Number(bgToneInput.value);
let avatarScale = Number(avatarScaleInput.value);
let pixelMode = "day";

const MIN_PIXEL_SIZE = 0;
const MAX_PIXEL_SIZE = 25;
const MIN_THRESHOLD = 100;
const MAX_THRESHOLD = 200;
const MIN_BG_TONE = 0;
const MAX_BG_TONE = 100;
const MIN_AVATAR_SCALE = 10;
const MAX_AVATAR_SCALE = 100;

const DAY_BG_MIN = 215;
const DAY_BG_MAX = 252;
const NIGHT_BG_MIN = 32;
const NIGHT_BG_MAX = 70;

function frameSourceKey(role, expression, direction) {
  return `${role}:${expression}:${direction}`;
}

function loadImages() {
  const entries = [];

  Object.entries(expressionSources).forEach(([role, expressions]) => {
    Object.entries(expressions).forEach(([expression, sources]) => {
      Object.entries(sources).forEach(([direction, src]) => {
        entries.push([frameSourceKey(role, expression, direction), src]);
      });
    });
  });

  return Promise.all(
    entries.map(([key, src]) => {
      return new Promise((resolve) => {
        const image = new Image();
        const done = () => {
          images[key] = image;
          resolve();
        };
        image.onload = done;
        image.onerror = () => {
          console.warn(`Image failed to load: ${src}`);
          done();
        };
        image.src = src;
      });
    }),
  );
}

function drawBlackWhitePixelArt(smallWidth, smallHeight) {
  const imageData = offscreenContext.getImageData(0, 0, smallWidth, smallHeight);
  const data = imageData.data;
  const isNight = pixelMode === "night";
  const transparentValue = isNight ? 0 : 255;

  for (let index = 0; index < data.length; index += 4) {
    const gray = data[index] * 0.299 + data[index + 1] * 0.587 + data[index + 2] * 0.114;
    const value = gray < threshold ? 0 : 255;

    data[index] = value;
    data[index + 1] = value;
    data[index + 2] = value;
    data[index + 3] = value === transparentValue ? 0 : 255;
  }

  offscreenContext.putImageData(imageData, 0, 0);
}

function clamp(value, min, max) {
  return Math.min(Math.max(value, min), max);
}

function grayTone(isNight, toneValue) {
  const ratio = clamp(toneValue, MIN_BG_TONE, MAX_BG_TONE) / 100;
  const min = isNight ? NIGHT_BG_MIN : DAY_BG_MIN;
  const max = isNight ? NIGHT_BG_MAX : DAY_BG_MAX;

  return Math.round(min + (max - min) * ratio);
}

function toRgb(value) {
  return `rgb(${value}, ${value}, ${value})`;
}

function applyBackgroundTheme() {
  const isNight = pixelMode === "night";
  const base = grayTone(isNight, bgTone);
  document.body.style.background = toRgb(base);
}

function applyAvatarScale() {
  avatarScale = clamp(avatarScale, MIN_AVATAR_SCALE, MAX_AVATAR_SCALE);
  avatarScaleInput.value = String(Math.round(avatarScale));
  avatarScaleValue.value = String(Math.round(avatarScale));
}

function getFallbackImage() {
  const fallbackExpression = Object.keys(expressionSources[currentRole] || {})[0] || "panic";
  return images[frameSourceKey(currentRole, fallbackExpression, "center")] || null;
}

function drawAvatar() {
  const image = images[frameSourceKey(currentRole, currentExpression, currentDirection)] || getFallbackImage();

  if (!image) return;

  context.clearRect(0, 0, canvas.width, canvas.height);
  context.fillStyle = toRgb(grayTone(pixelMode === "night", bgTone));
  context.fillRect(0, 0, canvas.width, canvas.height);

  const useIdentityPixelSize = pixelSize <= 0;
  const smallWidth = useIdentityPixelSize
    ? canvas.width
    : Math.max(1, Math.floor(canvas.width / pixelSize));
  const smallHeight = useIdentityPixelSize
    ? canvas.height
    : Math.max(1, Math.floor(canvas.height / pixelSize));

  offscreen.width = smallWidth;
  offscreen.height = smallHeight;
  offscreenContext.imageSmoothingEnabled = true;
  offscreenContext.clearRect(0, 0, smallWidth, smallHeight);
  offscreenContext.drawImage(image, 0, 0, smallWidth, smallHeight);
  drawBlackWhitePixelArt(smallWidth, smallHeight);

  const basePixelWidth = useIdentityPixelSize
    ? canvas.width
    : smallWidth * pixelSize;
  const basePixelHeight = useIdentityPixelSize
    ? canvas.height
    : smallHeight * pixelSize;
  const scaleFactor = avatarScale / 100;
  const drawWidth = Math.max(1, Math.round(basePixelWidth * scaleFactor));
  const drawHeight = Math.max(1, Math.round(basePixelHeight * scaleFactor));
  const offsetX = Math.max(0, Math.round((canvas.width - drawWidth) / 2));
  const offsetY = Math.max(0, Math.round((canvas.height - drawHeight) / 2));

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

function directionFromPointer(clientX, clientY) {
  const rect = canvas.getBoundingClientRect();
  const centerX = rect.left + rect.width / 2;
  const centerY = rect.top + rect.height / 2;
  const dx = clientX - centerX;
  const dy = clientY - centerY;
  const distance = Math.hypot(dx, dy);

  if (distance < rect.width * 0.12) {
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

function setExpression(expression) {
  if (!expressionSources[currentRole] || !expressionSources[currentRole][expression]) {
    return;
  }

  currentExpression = expression;

  if (emojiMapSelect) {
    emojiMapSelect.value = expression;
  }

  if (expressionButtonsContainer) {
    const buttons = expressionButtonsContainer.querySelectorAll(".expression-button");
    buttons.forEach((button) => {
      button.classList.toggle("is-active", button.dataset.expression === expression);
    });
  }

  drawAvatar();
}

function renderExpressionGrid() {
  if (!expressionButtonsContainer) return;

  expressionButtonsContainer.innerHTML = "";
  const expressionsForRole = Object.keys(expressionSources[currentRole] || {});

  expressionsForRole.forEach((expression) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "expression-button";
    button.dataset.expression = expression;
    button.textContent = expressionLabels[expression] || expression;
    if (expression === currentExpression) {
      button.classList.add("is-active");
    }
    expressionButtonsContainer.append(button);
  });
}

function renderEmojiMap() {
  if (!emojiMapSelect) return;

  emojiMapSelect.innerHTML = "";
  const expressionsForRole = Object.keys(expressionSources[currentRole] || {});

  expressionsForRole.forEach((expression) => {
    const aliases = expressionAliases[expression];
    const option = document.createElement("option");
    option.value = expression;
    const label = expressionLabels[expression] || expression;
    option.textContent = aliases && aliases.length > 0 ? `${label} ${aliases.join(" / ")}` : label;
    emojiMapSelect.append(option);
  });
  emojiMapSelect.value = currentExpression;
}

function setRole(role) {
  if (!expressionSources[role]) {
    return;
  }

  currentRole = role;

  const availableExpressions = Object.keys(expressionSources[currentRole]);
  if (!availableExpressions.includes(currentExpression)) {
    currentExpression = availableExpressions[0];
  }

  roleButtons.forEach((button) => {
    button.classList.toggle("is-active", button.dataset.role === role);
  });
  renderExpressionGrid();
  renderEmojiMap();
  setExpression(currentExpression);
}

function setMode(mode) {
  pixelMode = mode;
  modeButtons.forEach((button) => {
    button.classList.toggle("is-active", button.dataset.mode === mode);
  });
  document.body.dataset.mode = mode;
  applyBackgroundTheme();
  drawAvatar();
}

window.addEventListener("pointermove", (event) => {
  const nextDirection = directionFromPointer(event.clientX, event.clientY);

  if (nextDirection === currentDirection) return;
  currentDirection = nextDirection;
  drawAvatar();
});

window.addEventListener("pointerleave", () => {
  currentDirection = "center";
  drawAvatar();
});

pixelSizeInput.addEventListener("input", () => {
  pixelSize = Number(pixelSizeInput.value);
  pixelSize = Math.min(Math.max(pixelSize, MIN_PIXEL_SIZE), MAX_PIXEL_SIZE);
  pixelSizeInput.value = String(pixelSize);
  pixelValue.value = String(pixelSize);
  drawAvatar();
});

thresholdInput.addEventListener("input", () => {
  threshold = Number(thresholdInput.value);
  threshold = Math.min(Math.max(threshold, MIN_THRESHOLD), MAX_THRESHOLD);
  thresholdInput.value = String(threshold);
  thresholdValue.value = String(threshold);
  drawAvatar();
});

bgToneInput.addEventListener("input", () => {
  bgTone = Number(bgToneInput.value);
  bgTone = clamp(bgTone, MIN_BG_TONE, MAX_BG_TONE);
  bgToneInput.value = String(bgTone);
  bgToneValue.value = String(bgTone);
  applyBackgroundTheme();
  drawAvatar();
});

avatarScaleInput.addEventListener("input", () => {
  avatarScale = Number(avatarScaleInput.value);
  applyAvatarScale();
  drawAvatar();
});

if (expressionButtonsContainer) {
  expressionButtonsContainer.addEventListener("click", (event) => {
    const button = event.target.closest(".expression-button");
    if (!button || !expressionButtonsContainer.contains(button)) {
      return;
    }
    setExpression(button.dataset.expression);
  });
}

if (emojiMapSelect) {
  emojiMapSelect.addEventListener("change", () => {
    const next = emojiMapSelect.value;
    if (expressionSources[currentRole][next]) {
      setExpression(next);
    }
  });
}

roleButtons.forEach((button) => {
  button.addEventListener("click", () => {
    setRole(button.dataset.role);
  });
});

modeButtons.forEach((button) => {
  button.addEventListener("click", () => {
    setMode(button.dataset.mode);
  });
});

setRole(currentRole);

loadImages().then(() => {
  document.body.dataset.mode = pixelMode;
  applyBackgroundTheme();
  applyAvatarScale();
  drawAvatar();
});
