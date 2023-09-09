@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> @builtin(position) vec4<f32> {
    var verts = array<vec2f, 6>(vec2f(1.0, 1.0), vec2f(-1.0, 1.0), vec2f(-1.0, -1.0), vec2f(1.0, 1.0), vec2f(-1.0, -1.0), vec2f(1.0, -1.0));
    let v = verts[in_vertex_index];
    return vec4<f32>(v.x, v.y, 0.0, 1.0);
}