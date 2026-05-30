const canvas = document.querySelector("#avatar");
const context = canvas.getContext("2d");
const pixelSizeInput = document.querySelector("#pixelSize");
const pixelValue = document.querySelector("#pixelValue");
const thresholdInput = document.querySelector("#threshold");
const thresholdValue = document.querySelector("#thresholdValue");
const bgToneInput = document.querySelector("#bgTone");
const bgToneValue = document.querySelector("#bgToneValue");
const expressionButtons = document.querySelectorAll(".expression-button");
const modeButtons = document.querySelectorAll(".mode-button");

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

const expressionSources = {
  panic: {
    center: "./assets/look-center.png",
    up: "./assets/look-up.png",
    down: "./assets/look-down.png",
    left: "./assets/look-left.png",
    right: "./assets/look-right.png",
    "up-left": "./assets/look-up-left.png",
    "up-right": "./assets/look-up-right.png",
    "down-left": "./assets/look-down-left.png",
    "down-right": "./assets/look-down-right.png",
  },
  crying: {},
  confused: {},
  nervous: {},
};

["crying", "confused", "nervous"].forEach((expression) => {
  directions.forEach((direction) => {
    expressionSources[expression][direction] = `./assets/${expression}-${direction}.png`;
  });
});

const images = {};
const offscreen = document.createElement("canvas");
const offscreenContext = offscreen.getContext("2d");

let currentDirection = "center";
let currentExpression = "panic";
let pixelSize = Number(pixelSizeInput.value);
let threshold = Number(thresholdInput.value);
let bgTone = Number(bgToneInput.value);
let pixelMode = "day";

const MIN_PIXEL_SIZE = 5;
const MAX_PIXEL_SIZE = 25;
const MIN_THRESHOLD = 100;
const MAX_THRESHOLD = 200;
const MIN_BG_TONE = 0;
const MAX_BG_TONE = 100;

const DAY_BG_MIN = 215;
const DAY_BG_MAX = 252;
const NIGHT_BG_MIN = 32;
const NIGHT_BG_MAX = 70;

function sourceKey(expression, direction) {
  return `${expression}:${direction}`;
}

function loadImages() {
  const entries = [];

  Object.entries(expressionSources).forEach(([expression, sources]) => {
    Object.entries(sources).forEach(([direction, src]) => {
      entries.push([sourceKey(expression, direction), src]);
    });
  });

  return Promise.all(
    entries.map(([key, src]) => {
      return new Promise((resolve) => {
        const image = new Image();
        image.onload = () => {
          images[key] = image;
          resolve();
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
    let value = gray < threshold ? 0 : 255;

    data[index] = value;
    data[index + 1] = value;
    data[index + 2] = value;
    data[index + 3] = value === transparentValue ? 0 : 255;
  }

  offscreenContext.putImageData(imageData, 0, 0);
  context.imageSmoothingEnabled = false;
  context.drawImage(offscreen, 0, 0, smallWidth, smallHeight, 0, 0, canvas.width, canvas.height);
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
  const high = toRgb(clamp(base + (isNight ? 22 : 10), 0, 255));
  const low = toRgb(clamp(base - (isNight ? 8 : 16), 0, 255));
  const center = toRgb(clamp(base + (isNight ? 14 : 18), 0, 255));

  document.body.style.background =
    `radial-gradient(circle at 50% 50%, ${center} 0 18%, transparent 19%), linear-gradient(135deg, ${high} 0%, ${low} 100%)`;
}

function drawAvatar() {
  const image = images[sourceKey(currentExpression, currentDirection)] || images[sourceKey("panic", "center")];

  if (!image) return;

  context.clearRect(0, 0, canvas.width, canvas.height);
  context.fillStyle = toRgb(grayTone(pixelMode === "night", bgTone));
  context.fillRect(0, 0, canvas.width, canvas.height);

  const smallWidth = Math.max(1, Math.round(canvas.width / pixelSize));
  const smallHeight = Math.max(1, Math.round(canvas.height / pixelSize));

  offscreen.width = smallWidth;
  offscreen.height = smallHeight;
  offscreenContext.imageSmoothingEnabled = true;
  offscreenContext.clearRect(0, 0, smallWidth, smallHeight);
  offscreenContext.drawImage(image, 0, 0, smallWidth, smallHeight);
  drawBlackWhitePixelArt(smallWidth, smallHeight);
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
  currentExpression = expression;
  expressionButtons.forEach((button) => {
    button.classList.toggle("is-active", button.dataset.mode === expression);
  });
  drawAvatar();
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

expressionButtons.forEach((button) => {
  button.addEventListener("click", () => {
    setExpression(button.dataset.mode);
  });
});

modeButtons.forEach((button) => {
  button.addEventListener("click", () => {
    setMode(button.dataset.mode);
  });
});

loadImages().then(() => {
  document.body.dataset.mode = pixelMode;
  applyBackgroundTheme();
  drawAvatar();
});
