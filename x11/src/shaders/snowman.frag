#version 300 es
// A snowman of three balls of packed snow, drawn from its age alone: the bottom ball rolls in along the drift
// (0-1 s), the middle one drops onto it (1-1.8 s), then the head (1.9-2.7 s), then coal eyes, a smile and
// buttons, a top hat, a carrot nose, a red scarf and twig arms (2.8-3.6 s). The balls' sizes come from `radii`, so
// three stacked by hand keep theirs; the face and the rest scale with the ball they are on. Premultiplied alpha.
precision highp float;
precision highp int;

uniform vec4 light;
// The panel's rectangle, left, bottom, right, top: the snowman goes behind it.
uniform vec4 panel;
uniform float age;
// How much of it is left as it crumbles.
uniform float fade;
uniform float seed;
// The bottom ball, the middle one and the head, as shares of the bottom one's radius (the unit here).
uniform vec3 radii;

in vec2 vLocal;
in vec2 vWorld;

out vec4 fragment;

const vec3 SUN = vec3(-0.45, 0.62, 0.64);
const vec3 COAL = vec3(0.07, 0.07, 0.08);
const vec3 CARROT = vec3(0.96, 0.46, 0.10);
const vec3 SCARF = vec3(0.80, 0.11, 0.15);
const vec3 TWIG = vec3(0.33, 0.22, 0.13);
const vec3 HAT = vec3(0.07, 0.07, 0.085);
const vec3 BAND = vec3(0.55, 0.06, 0.09);

float hash(vec2 p) {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

float noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

float bounce(float t) {
    if (t < 1.0 / 2.75) return 7.5625 * t * t;
    if (t < 2.0 / 2.75) { t -= 1.5 / 2.75; return 7.5625 * t * t + 0.75; }
    if (t < 2.5 / 2.75) { t -= 2.25 / 2.75; return 7.5625 * t * t + 0.9375; }
    t -= 2.625 / 2.75;
    return 7.5625 * t * t + 0.984375;
}

// Coverage of a shape whose signed distance is `d` (negative inside), smoothed over about a pixel.
float coverage(float d) {
    float w = max(fwidth(d), 1e-4);
    return clamp(0.5 - d / w, 0.0, 1.0);
}

float segment(vec2 p, vec2 a, vec2 b) {
    vec2 pa = p - a;
    vec2 ba = b - a;
    float h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
    return length(pa - ba * h);
}

vec4 over(vec4 front, vec4 back) {
    return front + back * (1.0 - front.a);
}

// A ball of packed snow at `c` of radius `r`, its grain turned by `roll` as it rolls; `shadow` darkens where
// the ball above rests on it.
vec4 ball(vec2 p, vec2 c, float r, float roll, vec2 above, float aboveR) {
    if (r <= 0.0) return vec4(0.0);
    vec2 d = (p - c) / r;
    float a = coverage((length(d) - 1.0) * r);
    if (a <= 0.0) return vec4(0.0);
    float z = sqrt(max(0.0, 1.0 - dot(d, d)));
    vec3 n = vec3(d, z);
    float diffuse = max(dot(n, normalize(SUN)), 0.0);
    float rim = pow(1.0 - z, 2.5);
    vec3 color = mix(vec3(0.62, 0.70, 0.86), vec3(0.98, 0.99, 1.0), 0.35 + 0.65 * diffuse);
    color = mix(color, vec3(0.80, 0.88, 1.0), rim * 0.35);
    float c0 = cos(roll), s0 = sin(roll);
    vec2 q = mat2(c0, -s0, s0, c0) * d;
    color *= 0.93 + 0.1 * noise(q * 6.0 + seed * 17.0) + 0.04 * noise(q * 19.0 - seed * 5.0);
    float glint = pow(max(dot(reflect(-normalize(SUN), n), vec3(0.0, 0.0, 1.0)), 0.0), 24.0);
    color += vec3(0.22) * glint;
    if (aboveR > 0.0) {
        vec2 contact = above - vec2(0.0, aboveR * 0.85);
        vec2 k = (p - contact) / (aboveR * 0.75);
        color *= 1.0 - 0.32 * exp(-dot(k, k) * 2.2);
    }
    return vec4(color * a, a);
}

vec4 dot2(vec2 p, vec2 c, float r, vec3 color) {
    if (r <= 0.0) return vec4(0.0);
    float a = coverage(length(p - c) - r);
    vec2 d = (p - c) / r;
    vec3 shade = color + vec3(0.18) * max(0.0, 1.0 - length(d - vec2(-0.35, 0.4)) * 1.6);
    return vec4(shade * a, a);
}

void main() {
    if (vWorld.x >= panel.x && vWorld.x < panel.z && vWorld.y >= panel.y && vWorld.y < panel.w) discard;
    vec2 p = vLocal;

    // Bottom ball: rolled in from the left, growing as it gathers snow.
    float t0 = clamp(age / 1.0, 0.0, 1.0);
    float e0 = 1.0 - pow(1.0 - t0, 3.0);
    float r0 = mix(0.35, 1.0, e0) * radii.x;
    float x0 = mix(-2.6, 0.0, e0);
    vec2 c0 = vec2(x0, 0.92 * r0);
    // Where the middle ball and the head end up, and how much bigger than the menu's snowman's each is.
    float restM = 0.92 * radii.x + 0.85 * (radii.x + radii.y);
    float restH = restM + 0.85 * (radii.y + radii.z);
    float km = radii.y / 0.70;
    float k = radii.z / 0.48;
    float roll0 = -x0 / max(r0, 0.35);

    // Middle ball and head: dropped onto the one under, with a bounce.
    float rm = radii.y;
    float t1 = clamp((age - 1.0) / 0.8, 0.0, 1.0);
    vec2 cm = vec2(0.0, restM + (1.0 - bounce(t1)) * 1.8);
    float am = smoothstep(0.0, 0.12, t1);
    float rh = radii.z;
    float t2 = clamp((age - 1.9) / 0.8, 0.0, 1.0);
    vec2 ch = vec2(0.0, restH + (1.0 - bounce(t2)) * 1.8);
    float ah = smoothstep(0.0, 0.12, t2);

    // The face, the buttons, the nose, the scarf and the arms.
    float tf = clamp((age - 2.8) / 0.8, 0.0, 1.0);
    float pop = smoothstep(0.0, 0.5, tf);
    float grow = smoothstep(0.2, 1.0, tf);

    vec4 color = vec4(0.0);
    // Twig arms, behind the middle ball.
    if (grow > 0.0) {
        float w = 0.04 * km;
        vec2 la = cm + vec2(-0.6, 0.12) * km, lb = cm + vec2(-1.55, 0.74) * km, lf = cm + vec2(-1.47, 1.02) * km;
        vec2 ra = cm + vec2(0.6, 0.10) * km, rb = cm + vec2(1.5, 0.52) * km, rf = cm + vec2(1.55, 0.82) * km;
        vec2 lEnd = mix(la, lb, grow), rEnd = mix(ra, rb, grow);
        float d = min(segment(p, la, lEnd), segment(p, ra, rEnd));
        vec2 lFork = mix(la, lb, 0.72), rFork = mix(ra, rb, 0.7);
        if (grow > 0.72) d = min(d, segment(p, lFork, mix(lFork, lf, (grow - 0.72) / 0.28)));
        if (grow > 0.70) d = min(d, segment(p, rFork, mix(rFork, rf, (grow - 0.70) / 0.30)));
        float a = coverage(d - w);
        color = over(vec4(TWIG * a, a), color);
    }
    color = over(ball(p, c0, r0, roll0, cm, t1 > 0.0 ? rm : 0.0), color);
    color = over(ball(p, cm, rm, seed, ch, t2 > 0.0 ? rh : 0.0) * am, color);
    color = over(ball(p, ch, rh, seed * 2.0, vec2(0.0), 0.0) * ah, color);

    if (tf > 0.0) {
        // A red scarf at the neck, its end hanging down the right.
        float neck = ch.y - rh * 0.82;
        float band = abs(p.y - neck - 0.03 * p.x) - 0.085 * k;
        float scarf = max(band, abs(p.x) - 0.49 * k);
        vec2 tail0 = vec2(0.20 * k, neck - 0.02 * k), tail1 = vec2(0.34 * k, neck - 0.42 * k);
        scarf = min(scarf, segment(p, tail0, tail1) - 0.075 * k);
        float stripes = step(0.5, fract((p.x + p.y) * 6.0 / k));
        vec3 wool = mix(SCARF, SCARF * 0.78, stripes * 0.6);
        float a = coverage(scarf) * pop;
        color = over(vec4(wool * a, a), color);
        // Coal buttons down the front.
        float rb = 0.06 * pop * km;
        color = over(dot2(p, cm + vec2(0.02, 0.32) * km, rb, COAL), color);
        color = over(dot2(p, cm + vec2(0.03, 0.02) * km, rb, COAL), color);
        color = over(dot2(p, c0 + vec2(0.03, 0.52 * radii.x), rb, COAL), color);
        // Eyes and a smile of coal.
        color = over(dot2(p, ch + vec2(-0.16, 0.11) * k, 0.055 * pop * k, COAL), color);
        color = over(dot2(p, ch + vec2(0.17, 0.11) * k, 0.055 * pop * k, COAL), color);
        for (int i = 0; i < 5; i++) {
            float x = (float(i) - 2.0) * 0.095;
            color = over(dot2(p, ch + vec2(x + 0.02, -0.21 + 0.07 * (x / 0.19) * (x / 0.19)) * k, 0.028 * pop * k, COAL), color);
        }
        // A black top hat with a red band, dropped onto the head.
        float th = clamp((age - 2.8) / 0.6, 0.0, 1.0);
        float drop = (1.0 - bounce(th)) * 0.9 * k;
        float brimY = ch.y + 0.36 * k + drop;
        vec2 hb = p - vec2(0.0, brimY);
        float brim = max(abs(hb.x) - 0.36 * k, abs(hb.y) - 0.032 * k);
        vec2 hc = p - vec2(0.0, brimY + 0.22 * k);
        float crown = max(abs(hc.x) - (0.23 - 0.02 * (hc.y / (0.22 * k))) * k, abs(hc.y) - 0.22 * k);
        float hat = min(brim, crown);
        float ha = coverage(hat) * smoothstep(0.0, 0.1, th);
        float ribbon = step(brimY + 0.035 * k, p.y) * step(p.y, brimY + 0.1 * k);
        vec3 felt = mix(HAT, BAND, ribbon) + vec3(0.07) * smoothstep(0.1 * k, -0.2 * k, p.x) * step(0.0, crown - brim);
        color = over(vec4(felt * ha, ha), color);
        // A carrot nose pointing right, tapering to its tip.
        vec2 n0 = ch + vec2(0.03, -0.03) * k;
        vec2 n1 = n0 + vec2(0.44, -0.05) * grow * k;
        vec2 pa = p - n0, ba = n1 - n0;
        float h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
        float nose = length(pa - ba * h) - mix(0.07, 0.008, h) * k;
        float an = coverage(nose) * step(0.01, grow);
        vec3 carrot = CARROT * (0.85 + 0.25 * step(0.5, fract(h * 5.0)));
        color = over(vec4(carrot * an, an), color);
    }

    color.rgb *= mix(vec3(1.0), light.rgb, 0.85);
    fragment = color * fade;
}
