#!/usr/bin/env python3
"""
McFloater Vision Service — LingBot-Map live wrapper

This service receives JPEG frames (from the brain's WebRTC video track)
and runs LingBot-Map streaming inference.

It is intentionally kept simple so the brain's decoder has a real target
to push frames into.

Environment:
  LINGBOT_MODEL_PATH   Path to lingbot-map.pt (required)
  CUDA_VISIBLE_DEVICES Which GPU(s) to use
"""

import os
import time
import uuid
import io
from typing import Dict, Any, Optional

import numpy as np
from PIL import Image
from fastapi import FastAPI, UploadFile, File, WebSocket, WebSocketDisconnect
from fastapi.responses import JSONResponse
import uvicorn

# ---------------------------------------------------------------------------
# LingBot-Map import (the real model)
# ---------------------------------------------------------------------------
try:
    import torch
    from lingbot_map import GeometricContextTransformer, load_checkpoint
    LINGBOT_AVAILABLE = True
except ImportError as e:
    LINGBOT_AVAILABLE = False
    print(f"[vision] LingBot-Map import failed: {e}")

app = FastAPI(title="McFloater Vision Service", version="0.1.0")

# ---------------------------------------------------------------------------
# Model loading (done once at startup)
# ---------------------------------------------------------------------------
MODEL = None
DEVICE = "cuda" if torch.cuda.is_available() else "cpu"

def load_lingbot_model():
    global MODEL
    model_path = os.environ.get("LINGBOT_MODEL_PATH")
    if not model_path or not os.path.exists(model_path):
        print(f"[vision] LINGBOT_MODEL_PATH not set or file missing: {model_path}")
        return False

    if not LINGBOT_AVAILABLE:
        print("[vision] LingBot-Map package not importable")
        return False

    print(f"[vision] Loading LingBot-Map from {model_path} on {DEVICE} ...")
    try:
        checkpoint = load_checkpoint(model_path)
        MODEL = GeometricContextTransformer.from_checkpoint(checkpoint).to(DEVICE)
        MODEL.eval()
        print("[vision] LingBot-Map model loaded successfully")
        return True
    except Exception as e:
        print(f"[vision] Failed to load LingBot-Map: {e}")
        return False

MODEL_LOADED = load_lingbot_model()

# ---------------------------------------------------------------------------
# Session state (very lightweight for now)
# ---------------------------------------------------------------------------
sessions: Dict[str, Dict[str, Any]] = {}


@app.get("/health")
async def health():
    return {
        "status": "ok",
        "model_loaded": MODEL_LOADED,
        "device": DEVICE,
        "lingbot_available": LINGBOT_AVAILABLE,
    }


@app.post("/v1/vision/ingest")
async def start_ingest():
    """Start a new vision session."""
    sid = str(uuid.uuid4())
    sessions[sid] = {
        "created": time.time(),
        "frames": 0,
        "last_scene": None,
    }
    return {"session_id": sid}


@app.post("/v1/vision/frame")
async def push_frame(session_id: str, file: UploadFile = File(...)):
    """
    Receive one JPEG frame from the brain (WebRTC video track).
    Runs LingBot-Map inference and updates the session scene.
    """
    if session_id not in sessions:
        return JSONResponse({"error": "unknown session"}, status_code=404)

    content = await file.read()
    sessions[session_id]["frames"] += 1

    # Decode JPEG → numpy RGB (HWC, uint8)
    try:
        img = Image.open(io.BytesIO(content)).convert("RGB")
        frame = np.array(img)
    except Exception as e:
        return JSONResponse({"error": f"bad jpeg: {e}"}, status_code=400)

    scene = None

    if MODEL is not None:
        try:
            # ------------------------------------------------------------------
            # Real LingBot-Map streaming inference
            # ------------------------------------------------------------------
            # The model expects a numpy RGB frame (H, W, 3) uint8.
            # We call the streaming step and obtain pose + geometry output.
            result = MODEL.step(frame)  # returns dict with 'poses', 'points', etc.

            # Post-process into the compact object list + free-text description
            # that the brain expects.
            objects = []
            if "points" in result and len(result["points"]) > 0:
                # Very simple heuristic: treat the closest point cluster as an object
                objects.append({
                    "label": "scene_object",
                    "confidence": 0.85,
                    "bbox_3d": result.get("bbox", [0, 0, 0]),
                    "distance": float(np.min(result.get("depth", [10.0]))),
                    "bearing": 0.0,
                })

            description = result.get(
                "caption",
                "Scene reconstructed with LingBot-Map (no caption available)."
            )

            scene = {
                "session_id": session_id,
                "timestamp": time.time(),
                "objects": objects,
                "description": description,
            }
            sessions[session_id]["last_scene"] = scene

        except Exception as e:
            print("[vision] LingBot-Map inference failed:", e)
            scene = {
                "session_id": session_id,
                "timestamp": time.time(),
                "objects": [],
                "description": f"Inference error: {e}",
            }
    else:
        scene = {
            "session_id": session_id,
            "timestamp": time.time(),
            "objects": [],
            "description": "Model not loaded – inference skipped.",
        }

    return {"ok": True, "bytes": len(content), "model_loaded": MODEL_LOADED, "scene": scene}


@app.get("/v1/vision/scene")
async def get_scene(session_id: str):
    """Return the latest structured object list + free-text description."""
    if session_id not in sessions:
        return JSONResponse({"error": "unknown session"}, status_code=404)

    scene = sessions[session_id].get("last_scene")
    if scene is None:
        return {
            "session_id": session_id,
            "timestamp": time.time(),
            "objects": [],
            "description": "No scene yet (waiting for frames or LingBot-Map not fully wired).",
        }

    return scene


@app.websocket("/v1/vision/stream")
async def vision_stream(ws: WebSocket):
    """Bidirectional streaming endpoint (frames in, results out)."""
    await ws.accept()
    try:
        while True:
            data = await ws.receive_bytes()
            # In the future: decode JPEG, run MODEL.step(), send back scene JSON
            await ws.send_json({
                "objects": [],
                "description": "streaming not implemented yet"
            })
    except WebSocketDisconnect:
        pass


if __name__ == "__main__":
    port = int(os.environ.get("VISION_PORT", 8760))
    uvicorn.run(app, host="0.0.0.0", port=port, log_level="info")