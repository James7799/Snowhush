#version 300 es
// A snowball made by hand: a quad just around it.
precision highp float;
precision highp int;

uniform vec2 viewSize;
uniform vec2 shake;
uniform vec2 center;
uniform float radius;

// Where the pixel is from the ball's centre, in radii.
out vec2 vLocal;
out vec2 vWorld;

void main() {
    vec2 corner = vec2(float(gl_VertexID & 1), float(gl_VertexID >> 1));
    vec2 local = (corner * 2.0 - 1.0) * 1.08;
    vec2 world = center + local * radius;
    gl_Position = vec4((world + shake) / viewSize * 2.0 - 1.0, 0.0, 1.0);
    vLocal = local;
    vWorld = world;
}
