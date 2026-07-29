// Simple vertex + fragment shader for the headless avatar renderer.
// The fragment shader draws a stylized head and animates the mouth
// based on the uniform values provided by the brain.

struct Uniforms {
    mouth_open: f32,
    jaw: f32,
    lip_round: f32,
    brow: f32,
    _pad: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@location(0) position: vec2<f32>, @location(1) uv: vec2<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;

    // Simple head shape
    let dx = (uv.x - 0.5) * 2.0;
    let dy = (uv.y - 0.5) * 2.0;
    let dist = sqrt(dx*dx + dy*dy);

    if (dist > 0.85) {
        return vec4<f32>(0.08, 0.08, 0.12, 1.0); // background
    }

    // Skin
    var color = vec3<f32>(0.86, 0.71, 0.63);

    // Mouth
    let mouth_y = 0.55 + uniforms.jaw * 0.03;
    let mouth_half_width = 0.12;
    let mouth_half_height = 0.02 + uniforms.mouth_open * 0.06;

    if (abs(uv.x - 0.5) < mouth_half_width && abs(uv.y - mouth_y) < mouth_half_height) {
        color = vec3<f32>(0.15, 0.08, 0.08);
    }

    // Eyes (simple)
    if (abs(uv.x - 0.35) < 0.04 && abs(uv.y - 0.38) < 0.025) {
        color = vec3<f32>(0.1, 0.1, 0.15);
    }
    if (abs(uv.x - 0.65) < 0.04 && abs(uv.y - 0.38) < 0.025) {
        color = vec3<f32>(0.1, 0.1, 0.15);
    }

    return vec4<f32>(color, 1.0);
}