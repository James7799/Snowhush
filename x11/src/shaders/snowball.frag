#version 300 es
// A ball of packed snow, lit from the upper left like the snowmen's, its grain turned as it rolls. Premultiplied alpha.
precision highp float;
precision highp int;

uniform vec4 light;
uniform vec4 panel;
uniform float radius;
uniform float roll;

in vec2 vLocal;
in vec2 vWorld;

out vec4 fragment;

const vec3 SUN = vec3(-0.45, 0.62, 0.64);

float hash(vec2 p) {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

float noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

void main() {
    if (vWorld.x >= panel.x && vWorld.x < panel.z && vWorld.y >= panel.y && vWorld.y < panel.w) discard;
    vec2 d = vLocal;
    float dist = (length(d) - 1.0) * radius;
    float a = clamp(0.5 - dist / max(fwidth(dist), 1e-4), 0.0, 1.0);
    if (a <= 0.0) discard;
    float z = sqrt(max(0.0, 1.0 - dot(d, d)));
    vec3 n = vec3(d, z);
    float diffuse = max(dot(n, normalize(SUN)), 0.0);
    float rim = pow(1.0 - z, 2.5);
    vec3 color = mix(vec3(0.62, 0.70, 0.86), vec3(0.98, 0.99, 1.0), 0.35 + 0.65 * diffuse);
    color = mix(color, vec3(0.80, 0.88, 1.0), rim * 0.35);
    float c0 = cos(roll), s0 = sin(roll);
    vec2 q = mat2(c0, -s0, s0, c0) * d;
    color *= 0.93 + 0.1 * noise(q * 6.0) + 0.04 * noise(q * 19.0);
    float glint = pow(max(dot(reflect(-normalize(SUN), n), vec3(0.0, 0.0, 1.0)), 0.0), 24.0);
    color += vec3(0.22) * glint;
    color *= mix(vec3(1.0), light.rgb, 0.85);
    fragment = vec4(color * a, a);
}
