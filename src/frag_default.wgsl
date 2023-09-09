@group(0) @binding(0)
var<uniform> res: vec2<f32>;
@group(0) @binding(1)
var<uniform> frame: u32;
@group(0) @binding(2)
var videoBuffer: texture_2d<f32>;
@group(0) @binding(3)
var videoSampler: sampler;
@group(1) @binding(0)
var backBuffer: texture_2d<f32>;
@group(1) @binding(1)
var backSampler: sampler;

@fragment
fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let texcoords = pos.xy/res.xy;
    return vec4f(texcoords.x, texcoords.y, 0., 1.);
}