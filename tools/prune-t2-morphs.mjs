#!/usr/bin/env node
/**
 * Prune Avaturn T2 Head_Mesh morph targets to ≤64 so Bevy 0.15 can load the GLB.
 *
 *   npm i @gltf-transform/core @gltf-transform/extensions @gltf-transform/functions
 *   node tools/prune-t2-morphs.mjs assets/face/T2-avatar.glb assets/face/T2-avatar-bevy.glb
 *
 * Bevy MAX_MORPH_WEIGHTS = 64; raw T2 Head_Mesh has 72 ARKit+viseme targets.
 */
import { NodeIO } from '@gltf-transform/core';
import { ALL_EXTENSIONS } from '@gltf-transform/extensions';
import { prune } from '@gltf-transform/functions';
import fs from 'fs';

const KEEP = new Set([
  'mouthOpen', 'jawOpen', 'jawForward', 'jawLeft', 'jawRight',
  'mouthClose', 'mouthFunnel', 'mouthPucker', 'mouthSmile',
  'mouthSmileLeft', 'mouthSmileRight', 'mouthFrownLeft', 'mouthFrownRight',
  'mouthLeft', 'mouthRight', 'mouthLowerDownLeft', 'mouthLowerDownRight',
  'mouthUpperUpLeft', 'mouthUpperUpRight', 'mouthRollLower', 'mouthRollUpper',
  'mouthShrugLower', 'mouthShrugUpper', 'mouthPressLeft', 'mouthPressRight',
  'mouthStretchLeft', 'mouthStretchRight', 'mouthDimpleLeft', 'mouthDimpleRight',
  'cheekPuff', 'cheekSquintLeft', 'cheekSquintRight',
  'noseSneerLeft', 'noseSneerRight', 'tongueOut',
  'eyeBlinkLeft', 'eyeBlinkRight', 'eyesClosed',
  'eyeSquintLeft', 'eyeSquintRight', 'eyeWideLeft', 'eyeWideRight',
  'browDownLeft', 'browDownRight', 'browInnerUp', 'browOuterUpLeft', 'browOuterUpRight',
  'viseme_sil', 'viseme_PP', 'viseme_FF', 'viseme_TH', 'viseme_DD', 'viseme_kk',
  'viseme_CH', 'viseme_SS', 'viseme_nn', 'viseme_RR',
  'viseme_aa', 'viseme_E', 'viseme_I', 'viseme_O', 'viseme_U',
]);

const inPath = process.argv[2];
const outPath = process.argv[3] || inPath.replace(/\.glb$/i, '-bevy.glb');
if (!inPath) {
  console.error('Usage: node prune-t2-morphs.mjs <in.glb> [out.glb]');
  process.exit(1);
}

const io = new NodeIO().registerExtensions(ALL_EXTENSIONS);
const doc = await io.read(inPath);

for (const mesh of doc.getRoot().listMeshes()) {
  const meshName = mesh.getName() || '';
  const extras = { ...(mesh.getExtras() || {}) };
  const targetNames = extras.targetNames;
  if (!Array.isArray(targetNames)) continue;

  for (const prim of mesh.listPrimitives()) {
    const targets = prim.listTargets();
    if (!targets.length || targetNames.length !== targets.length) continue;
    if (targets.length <= 64) {
      console.log(`${meshName}: ${targets.length} ok`);
      continue;
    }
    const keptNames = [];
    for (let i = 0; i < targets.length; i++) {
      const n = targetNames[i];
      if (KEEP.has(n) && keptNames.length < 64) keptNames.push(n);
      else targets[i].dispose();
    }
    extras.targetNames = keptNames;
    mesh.setExtras(extras);
    console.log(`${meshName}: ${targetNames.length} → ${keptNames.length}`);
  }
}

await doc.transform(prune());
await io.write(outPath, doc);
console.log('wrote', outPath, `${(fs.statSync(outPath).size / 1e6).toFixed(2)} MB`);
