#version 300 es
// A snowman's quad: room for the bottom ball rolling in from the left, the arms, and the balls dropping onto it.
precision highp float;
precision highp int;

uniform vec2 viewSize;
// How far everything is jolted, in points.
uniform vec2 shake;
// The middle of its foot, in points, and its bottom ball's radius.
uniform vec2 anchor;
uniform float size;

// Where the pixel is from the foot, in bottom-ball radii, y up.
out vec2 vLocal;
out vec2 vWorld;

void main() {
    vec2 corner = vec2(float(gl_VertexID & 1), float(gl_VertexID >> 1));
    vec2 local = vec2(mix(-3.8, 2.0, corner.x), mix(-0.3, 5.9, corner.y));
    vec2 world = anchor + local * size;
    gl_Position = vec4((world + shake) / viewSize * 2.0 - 1.0, 0.0, 1.0);
    vLocal = local;
    vWorld = world;
}
