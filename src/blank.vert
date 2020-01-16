#version 450

layout(location = 0) out vec2 uv;
const mat4 verts = mat4(
    -1,1,0,1,
    1,1,0,1,
    -1,-1,0,1,
    1,-1,0,1
);

void main() {
    gl_Position = verts[gl_VertexIndex];
    uv = gl_Position.xy;
}
