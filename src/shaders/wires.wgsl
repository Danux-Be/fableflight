// Power line wires (power.rs): one instance per span, its ends relative to
// the camera, its sag at mid-span, radius, the local up and kind (0 a
// conductor, 1 the earth wire). The span is drawn as SEGS pieces along the
// parabola (near enough the catenary at these sags), each a ribbon turned to
// the camera, at least a pixel wide: where the wire is thinner than that the
// ribbon stays a pixel wide, lit as the wire would be on average across it;
// only once the wire covers under a third of that pixel is it thinned out,
// by a dither that TAA blends.
struct WOut {
  @builtin(position) clip: vec4f,
  @location(0) P: vec3f,
  @location(1) side: vec3f,
  // across the ribbon: -1..1
  @location(2) y: f32,
  @location(3) @interpolate(flat) cover: f32,
  @location(4) @interpolate(flat) kind: f32,
};
struct GOut { @location(0) c: vec4f, @location(1) n: vec4f, @location(2) amb: f32 };

const SEGS: u32 = 16u;

@vertex fn vs(@builtin(vertex_index) vi: u32, @location(0) a: vec4f, @location(1) b: vec4f, @location(2) u: vec4f) -> WOut {
  var corner = array<vec2f, 6>(vec2f(0.0, -1.0), vec2f(1.0, -1.0), vec2f(1.0, 1.0), vec2f(0.0, -1.0), vec2f(1.0, 1.0), vec2f(0.0, 1.0));
  let k = corner[vi % 6u];
  let t = (f32(vi / 6u) + k.x) / f32(SEGS);
  let up = u.xyz;
  let sag = a.w;
  let P = mix(a.xyz, b.xyz, t) - up * (sag * 4.0 * t * (1.0 - t));
  let T = normalize((b.xyz - a.xyz) - up * (sag * 4.0 * (1.0 - 2.0 * t)));
  let V = normalize(P);
  var side = cross(T, V);
  side = select(normalize(side), up, dot(side, side) < 1e-10);
  // Half a pixel at this distance (G.misc.w: the view's tangent per pixel).
  let px = 0.5 * G.misc.w * length(P);
  let r = b.w;
  let half = max(r, px);
  let Pw = P + side * (half * k.y);
  var o: WOut;
  o.clip = G.view_proj * vec4f(Pw, 1.0);
  o.P = Pw;
  o.side = side;
  o.y = k.y;
  o.cover = clamp(r / half, 0.0, 1.0);
  o.kind = u.w;
  return o;
}

@fragment fn fs(in: WOut) -> GOut {
  // Far thinner than its pixel: keep a share of the pixels (TAA blends them).
  let n = ign(in.clip.xy + vec2f(5.588238 * G.time.w, 3.1 * G.time.w));
  if (n > in.cover * 3.0) { discard; }
  let V = normalize(in.P);
  // (A ribbon only a pixel or two across shows the wire's average shading,
  // not its two sides pixel by pixel.)
  let y = clamp(in.y * in.cover, -0.98, 0.98);
  // A round wire: across the ribbon the normal turns from one side, through
  // facing the camera, to the other.
  let N = normalize(in.side * y - V * sqrt(1.0 - y * y));
  // Weathered aluminium conductors; the steel earth wire darker.
  let albedo = select(vec3f(0.30, 0.31, 0.32), vec3f(0.20, 0.21, 0.22), in.kind > 0.5);
  let l = light_at(in.P);
  var c = shade(in.P, N, albedo);
  // The sun's glint along the wire.
  let H = normalize(G.sun_dir.xyz - V);
  c += l.sun * pow(max(dot(N, H), 0.0), 60.0) * 0.25 * shadow_at(in.P, N);
  return GOut(vec4f(c, 1.0), vec4f(N, 0.0), amb_share(c));
}
