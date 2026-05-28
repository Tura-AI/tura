import assert from "node:assert/strict"
import { spawnSync } from "node:child_process"
import fs from "node:fs"
import path from "node:path"
import { fileURLToPath } from "node:url"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDir, "..", "..", "..", "..", "..")
const runRoot = path.join(repoRoot, "target", "command-run-read-media-e2e", String(Date.now()))
const turaExe = path.join(repoRoot, "target", "debug", process.platform === "win32" ? "tura.exe" : "tura")

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd || repoRoot,
    text: true,
    encoding: "utf8",
    maxBuffer: options.maxBuffer || 64 * 1024 * 1024,
    env: { ...process.env, ...(options.env || {}) },
  })
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with ${result.status}\nSTDOUT:\n${result.stdout}\nSTDERR:\n${result.stderr}`)
  }
  return result
}

function writeFixtures() {
  fs.mkdirSync(runRoot, { recursive: true })
  const py = String.raw`
from pathlib import Path
from PIL import Image, ImageDraw
from reportlab.pdfgen import canvas
import cv2
import numpy as np

root = Path(r"${runRoot.replaceAll("\\", "\\\\")}")

img = Image.new("RGB", (320, 180), "white")
d = ImageDraw.Draw(img)
d.rectangle([0, 0, 150, 180], fill=(220, 20, 20))
d.rectangle([170, 0, 320, 180], fill=(20, 80, 220))
d.text((78, 76), "RED", fill=(255, 255, 255))
d.text((230, 76), "BLUE", fill=(255, 255, 255))
img.save(root / "red_blue_panel.png")

pdf = canvas.Canvas(str(root / "media_brief.pdf"), pagesize=(320, 240))
pdf.setFont("Helvetica", 16)
pdf.drawString(30, 190, "Media Brief")
pdf.setFont("Helvetica", 11)
pdf.drawString(30, 160, "Subject: compact read_media validation.")
pdf.drawString(30, 140, "Figure: red left panel and blue right panel.")
pdf.drawString(30, 120, "Checklist: image, PDF text, and video frames.")
pdf.showPage()
pdf.save()

video_path = str(root / "color_steps.mp4")
out = cv2.VideoWriter(video_path, cv2.VideoWriter_fourcc(*"mp4v"), 2.0, (160, 120))
colors = [(0,0,255), (0,255,0), (255,0,0), (0,255,255), (255,0,255), (255,255,0)]
for idx, color in enumerate(colors):
    frame = np.zeros((120,160,3), dtype=np.uint8)
    frame[:,:] = color
    cv2.putText(frame, f"F{idx}", (48,72), cv2.FONT_HERSHEY_SIMPLEX, 1.2, (255,255,255), 2)
    out.write(frame)
out.release()
`
  run("python", ["-c", py], { cwd: repoRoot })
}

function parseJsonl(stdout) {
  return stdout
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      try {
        return JSON.parse(line)
      } catch {
        return null
      }
    })
    .filter(Boolean)
}

function collectText(events) {
  return events.map((event) => JSON.stringify(event)).join("\n")
}

function main() {
  writeFixtures()
  run("cargo", ["build", "-p", "gateway", "--bin", "tura"], { cwd: repoRoot })

  const prompt = [
    "Use command_run read_media to inspect these three local media files, then describe them accurately and concisely:",
    path.join(runRoot, "red_blue_panel.png"),
    path.join(runRoot, "media_brief.pdf"),
    path.join(runRoot, "color_steps.mp4"),
    "Mention image colors, the PDF subject/checklist text, and what the video frames show.",
  ].join("\n")

  const result = run(turaExe, [
    "exec",
    "--json",
    "--agent",
    "coding_agent_fast",
    "-m",
    process.env.COMMAND_RUN_AGENT_CODEX_MODEL || "gpt-5.1-codex",
    "-c",
    `model_reasoning_effort=${process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "low"}`,
    "-c",
    "service_tier=priority",
    "--cwd",
    runRoot,
    prompt,
  ], {
    cwd: runRoot,
    maxBuffer: 128 * 1024 * 1024,
    env: {
      TURA_COMMAND_RUN_SHELL: "shell_command",
      TURA_COMMAND_RUN_STRICT_JSON: "1",
      COMMAND_RUN_AGENT_TIMEOUT_MS: process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || "180000",
    },
  })

  fs.writeFileSync(path.join(runRoot, "tura-read-media.stdout.jsonl"), result.stdout)
  fs.writeFileSync(path.join(runRoot, "tura-read-media.stderr.log"), result.stderr)
  const events = parseJsonl(result.stdout)
  const text = collectText(events)
  const lower = text.toLowerCase()

  assert(lower.includes("read_media"), "agent should call read_media")
  assert(lower.includes("red") && lower.includes("blue"), "agent should describe image colors")
  assert(lower.includes("media brief") || lower.includes("compact read_media validation"), "agent should describe PDF text")
  assert(lower.includes("video") || lower.includes("frame"), "agent should describe video frames")
  assert(result.stdout.length < 6_000_000, `session stdout too large: ${result.stdout.length}`)

  const summary = {
    ok: true,
    run_root: runRoot,
    stdout_bytes: result.stdout.length,
    stderr_bytes: result.stderr.length,
    saw_read_media: lower.includes("read_media"),
    saw_image_description: lower.includes("red") && lower.includes("blue"),
    saw_pdf_description: lower.includes("media brief") || lower.includes("compact read_media validation"),
    saw_video_description: lower.includes("video") || lower.includes("frame"),
  }
  fs.writeFileSync(path.join(runRoot, "summary.json"), JSON.stringify(summary, null, 2))
  console.log(JSON.stringify(summary, null, 2))
}

main()
