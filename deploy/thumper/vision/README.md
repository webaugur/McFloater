# McFloater Vision Service (LingBot-Map)

This directory contains the Python service that runs LingBot-Map on live video frames coming from the browser via the brain's WebRTC endpoint.

## Quick start on Thumper

```bash
# 1. Create conda env (matches LingBot-Map recommendations)
conda create -n lingbot-map python=3.10 -y
conda activate lingbot-map

# 2. Install PyTorch + CUDA 12.8
pip install torch==2.8.0 torchvision==0.23.0 --index-url https://download.pytorch.org/whl/cu128

# 3. Install LingBot-Map + FlashInfer (recommended)
pip install -e /path/to/lingbot-map
pip install --index-url https://pypi.org/simple flashinfer-python

# 4. Install this service's dependencies
pip install -r requirements.txt

# 5. Set environment
cp env.vision.example ~/.config/mcfloater/vision.env
# edit the file: set LINGBOT_MODEL_PATH and CUDA_VISIBLE_DEVICES

# 6. Run the service directly (for testing)
VISION_PORT=8760 python vision_service.py
```

## Systemd unit

```bash
systemctl --user daemon-reload
systemctl --user enable --now mcfloater-vision
```

## Endpoints the brain uses

- `POST /v1/vision/ingest` → returns `session_id`
- `POST /v1/vision/frame` (multipart, field `file`) → one JPEG frame
- `GET  /v1/vision/scene?session_id=...` → latest objects + description

The brain's WebRTC video track decoder will call the `/frame` endpoint for every decoded frame.