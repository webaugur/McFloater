# Face assets (Avaturn GLB)

## Default (what McFloater loads)

| File | Why |
|------|-----|
| **`T2-avatar-with-animation-bevy.glb`** | **Default** — T2 face morphs (≤64 for Bevy) + embedded **`avaturn_animation`** clip (skeleton + weight tracks) |

Body pose comes from that **embedded animation** (looped). We do not invent arm Euler angles.

Override:

```bash
export MCFLOATER_FACE_GLB=face/T2-avatar-with-animation-bevy.glb  # default
# or without anim (T-pose body):
export MCFLOATER_FACE_GLB=face/T2-avatar-bevy.glb
```

## Your drops

| File | Morphs | Animation |
|------|--------|-----------|
| `T2-avatar.glb` | 72 head (too many for Bevy) | none |
| `T2-avatar-with-animation.glb` | 72 head | `avaturn_animation` |
| `T2-*-bevy.glb` | pruned ≤64 | same as source |
| `avatar.glb` / `avatar-with-animation.glb` | T1 static face | Facepalm on `*-with-animation` |

## Official Avaturn docs (not Euler guesses)

- [T1 vs T2 bodies](https://docs.avaturn.me/docs/integration/bodies/) — T2 = ARKit blendshapes + visemes  
- [Mixamo animations](https://docs.avaturn.me/docs/importing/mixamo/) — retarget body clips via their FBX workflow  
- [Blender import](https://docs.avaturn.me/docs/importing/blender/)

Face channels are standard ARKit names (`jawOpen`, `mouthOpen`, …) + Oculus visemes.

## Re-prune T2 for Bevy (64 morph limit)

```bash
node tools/prune-t2-morphs.mjs assets/face/T2-avatar-with-animation.glb \
  assets/face/T2-avatar-with-animation-bevy.glb
```
