@fragment
fn main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let texcoords = pos.xy/res.xy;
    return 0.5 * textureSample(videoBuffer, videoSampler, texcoords) + 0.5 * textureSample(backBuffer, backSampler, texcoords);

}
