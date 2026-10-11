#version 300 es
precision highp float;
// Three passes (the core says which): 0 solid, lit in flat bands like the
// TikZ art's fills; 1 ink, the plum outline, flat; 2 glow, an additive halo
// that is brightest where you look through the middle of a shape.
in vec3 vWorld;
in vec3 vNormal;
in vec4 vColor;
uniform vec3 uEye;
uniform vec3 uLightDir;
uniform vec3 uSky;
uniform vec3 uFog;
uniform vec2 uFogRange;
uniform vec3 uLight;
uniform vec3 uInk;
uniform float uPass;
out vec4 outColor;
void main() {
  vec3 n = normalize(vNormal);
  float d = length(vWorld - uEye);
  float f = clamp((d - uFogRange.x) / (uFogRange.y - uFogRange.x), 0.0, 1.0);
  f = f * f * (3.0 - 2.0 * f);
  if (uPass > 1.5) {
    vec3 v = normalize(uEye - vWorld);
    float through = abs(dot(n, v));
    float a = pow(through, 2.2) * vColor.a * (1.0 - 0.8 * f);
    outColor = vec4(vColor.rgb * a, 1.0);
    return;
  }
  if (uPass > 0.5) {
    outColor = vec4(mix(vColor.rgb, uFog, f * 0.85), 1.0);
    return;
  }
  vec3 base = vColor.rgb;
  float diffuse = dot(n, uLightDir);
  float band = diffuse > 0.5 ? 1.0 : (diffuse > 0.05 ? 0.8 : 0.62);
  float sky = 0.5 + 0.5 * n.y;
  vec3 ambient = mix(uFog * 0.6, uSky * 0.85, sky);
  vec3 lit = base * (ambient * 0.55 + uLight * band * 0.6);
  vec3 glow = base * 1.25 + vec3(0.08);
  lit = mix(lit, glow, vColor.a);
  outColor = vec4(mix(lit, uFog, f * (1.0 - 0.6 * vColor.a)), 1.0);
}
