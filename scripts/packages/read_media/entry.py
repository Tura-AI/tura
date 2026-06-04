import base64
import json
import sys

import cv2


def extract_video_frames(path: str, max_frames: int, max_side: int) -> list[str]:
    cap = cv2.VideoCapture(path)
    if not cap.isOpened():
        raise RuntimeError("failed to open video")
    total = int(cap.get(cv2.CAP_PROP_FRAME_COUNT) or 0)
    step = max(1, total // max(1, max_frames)) if total > 0 else 1
    frames: list[str] = []
    index = 0
    while len(frames) < max_frames:
        ok, frame = cap.read()
        if not ok:
            break
        if index % step == 0:
            height, width = frame.shape[:2]
            longest = max(width, height)
            if longest > max_side:
                scale = max_side / float(longest)
                frame = cv2.resize(frame, (max(1, int(width * scale)), max(1, int(height * scale))))
            ok, encoded = cv2.imencode(".jpg", frame, [int(cv2.IMWRITE_JPEG_QUALITY), 80])
            if ok:
                frames.append(base64.b64encode(encoded.tobytes()).decode("ascii"))
        index += 1
    cap.release()
    return frames


def main() -> None:
    request = json.load(sys.stdin)
    command = request.get("command")
    if command != "extract_video_frames":
        raise RuntimeError(f"unsupported command: {command}")
    frames = extract_video_frames(
        request["path"],
        int(request.get("max_frames", 6)),
        int(request.get("max_side", 512)),
    )
    print(json.dumps({"frames": frames}))


if __name__ == "__main__":
    main()
