// Instanced solid objects: cars, rubble, rocks.
@group(2) @binding(0) var<uniform> CS: Cascade;

struct VOut {
  @builtin(position) clip: vec4f,
  @location(0) P: vec3f,
  @location(1) N: vec3f,
  @location(2) col: vec4f,
  @location(3) L: vec3f,
  @location(4) @interpolate(flat) tint: vec4f,
  @location(5) Ln: vec3f,
  // pieces of facade: wall coordinates and style (see destruct.rs), and the
  // direction along the wall in which they grow
  @location(6) @interpolate(flat) ext: vec4f,
  @location(7) @interpolate(flat) T: vec3f,
};
struct GOut { @location(0) c: vec4f, @location(1) n: vec4f, @location(2) amb: f32 };

fn world(p: vec3f, w: f32, m0: vec4f, m1: vec4f, m2: vec4f) -> vec3f {
  let v = vec4f(p, w);
  return vec3f(dot(m0, v), dot(m1, v), dot(m2, v));
}

@vertex fn vs(@location(0) pos: vec3f, @location(1) nrm: vec3f, @location(2) col: vec4f,
              @location(3) m0: vec4f, @location(4) m1: vec4f, @location(5) m2: vec4f, @location(6) tint: vec4f, @location(7) ext: vec4f) -> VOut {
  let P = world(pos, 1.0, m0, m1, m2);
  // Scale may be uneven (rubble slabs): normals need the inverse scale.
  let sx = vec3f(length(vec3f(m0.x, m1.x, m2.x)), length(vec3f(m0.y, m1.y, m2.y)), length(vec3f(m0.z, m1.z, m2.z)));
  let N = world(nrm / max(sx * sx, vec3f(1e-6)), 0.0, m0, m1, m2);
  let T = normalize(vec3f(m0.x, m1.x, m2.x)) * sign(ext.z);
  return VOut(G.view_proj * vec4f(P, 1.0), P, N, col, pos * sx, tint, nrm, ext, T);
}
@vertex fn vs_shadow(@location(0) pos: vec3f, @location(3) m0: vec4f, @location(4) m1: vec4f, @location(5) m2: vec4f, @location(6) tint: vec4f, @location(7) ext: vec4f) -> @builtin(position) vec4f {
  // Street traffic hidden under the 3D tiles casts nothing (decided per car).
  if (tint.a < 0.5 && ext.w > 0.5 && photo_shadow_gone(vec3f(m0.w, m1.w, m2.w))) { return vec4f(2.0, 2.0, 0.0, 1.0); }
  return CS.vp * vec4f(world(pos, 1.0, m0, m1, m2), 1.0);
}

@fragment fn fs(in: VOut) -> GOut {
  // Street traffic (ext.w = 1) gives way to the 3D tiles (tiles3d.rs).
  if (in.tint.a < 0.5 && in.ext.w > 0.5 && photo_hides(in.P, in.clip.xy)) { discard; }
  var N = in.N;
  if (dot(N, N) < 1e-10) { N = -normalize(in.P); }
  N = normalize(N);
  let V = normalize(in.P);
  if (dot(N, V) > 0.0) { N = -N; }
  let l = light_at(in.P);
  let sh = shadow_at(in.P, N);
  let m = in.col.a;
  // Facade coordinates of a piece of outer wall: along the wall from the
  // building's own origin, and height above its ground (local -z is up).
  let fuv = vec2f(in.ext.x + sign(in.ext.z) * in.L.x, in.ext.y - in.L.z);
  let ffw = fwidth(fuv);
  // (How many metres of the surface a pixel spans, for the balloons'
  // patterns: taken here, in uniform control flow, as derivatives must be.)
  let dL = fwidth(in.L);
  let px = max(max(dL.x, dL.y), dL.z);
  var c: vec3f;
  var Nout = N;
  var mirror = 0.0;
  if (in.tint.a > 0.5) {
    // Matte rubble and rock, tinted per piece, with grit.
    let n = textureSampleLevel(t_noise, s_repeat, in.L * 0.23 + in.tint.rgb * 3.0, 0.0);
    let n2 = textureSampleLevel(t_noise, s_repeat, in.L * 1.1, 0.0);
    var albedo = in.col.rgb * in.tint.rgb * (0.55 + 0.9 * n.g) * (0.8 + 0.4 * n2.b);
    // Broken faces show darker insides.
    albedo = mix(albedo, albedo * 0.6, smoothstep(0.5, 0.85, n.b));
    // Faces of a fresh break (col.a = 1 on the broken-concrete meshes): grey
    // concrete whatever the facade was, coarse, with darker pits.
    let broken = m > 0.5 && m < 1.5;
    if (broken) {
      let n3 = textureSampleLevel(t_noise, s_repeat, in.L * 0.6 + 0.37, 0.0);
      let conc = vec3f(0.36, 0.35, 0.33) * mix(vec3f(1.0), in.tint.rgb / max(max(in.tint.r, in.tint.g), max(in.tint.b, 0.05)), 0.25);
      albedo = mix(albedo, in.col.rgb * conc * (0.6 + 0.8 * n3.r) * (0.75 + 0.5 * n2.g), 0.7);
      albedo *= 1.0 - 0.45 * smoothstep(0.62, 0.8, n3.g);
    }
    if (m > 1.5) {
      // Reinforcing bar: rusty steel, dusted.
      let n4 = textureSampleLevel(t_noise, s_repeat, in.P * 0.9, 0.0);
      albedo = mix(vec3f(0.085, 0.052, 0.036), vec3f(0.16, 0.15, 0.14), 0.25 * n4.r) * (0.75 + 0.5 * n4.g);
    }
    c = shade(in.P, N, albedo);
    // The outer face of a piece of facade (local +y) is drawn as the building drew it.
    if (in.tint.a > 1.5 && m < 0.5 && in.Ln.y > 0.5 && in.ext.z != 0.0) {
      let st = u32(in.ext.w);
      let fty = (st >> 8u) & 255u;
      let fs = facade_shade(in.P, V, N, in.T, fuv, ffw, in.col.rgb * in.tint.rgb, fty & 15u, f32(st & 255u) / 255.0, f32((st >> 16u) & 255u) / 255.0, abs(in.ext.z), (fty & 128u) != 0u, l, sh);
      c = shade(in.P, fs.N, fs.albedo) + fs.extra;
      Nout = fs.N;
      mirror = fs.mirror;
    }
  } else if (m > 1.15) {
    // Hot-air balloons (balloons.rs, balloons.py's MAT_CODE).
    let bs = balloon_surface(in, N, V, m, l, sh, px);
    c = bs.rgb;
    mirror = bs.a;
  } else {
    // Vehicle: m = 0 paint, 0.25 headlight, 0.5 glass, 0.75 tail light, 1 trim / tyre.
    let paint = step(m, 0.1);
    let glass = step(0.4, m) * step(m, 0.6);
    let head = step(0.15, m) * step(m, 0.35);
    let tail = step(0.65, m) * step(m, 0.85);
    var albedo = mix(in.col.rgb, in.tint.rgb, paint);
    let R = reflect(V, N);
    let ndv = max(dot(-V, N), 0.0);
    let f0 = paint * 0.05 + glass * 0.08 + head * 0.08;
    let fres = f0 + (1.0 - f0) * pow(1.0 - ndv, 5.0) * (paint * 0.6 + glass + head * 0.5);
    let Hh = normalize(G.sun_dir.xyz - V);
    let spec = pow(max(dot(N, Hh), 0.0), 220.0) * (paint + glass) * 2.5 * sh;
    c = shade(in.P, N, albedo * (1.0 - glass * 0.5));
    c += sky_lut(normalize(R + l.upn * 0.12)) * fres + l.sun * spec * 0.3;
    // Lights come on at dusk.
    let night = 1.0 - smoothstep(-0.05, 0.12, dot(l.upn, G.sun_dir.xyz));
    c += (head * vec3f(1.0, 0.95, 0.8) * 3.0 + tail * vec3f(1.0, 0.04, 0.02) * 0.9) * G.sun_dir.w * 0.06 * (0.15 + night);
  }
  return GOut(vec4f(c, 1.0), vec4f(Nout, -mirror), amb_share(c));
}

// ------------------------------------------------------------ hot-air balloons
// Materials by colour alpha (balloons.py MAT_CODE): 1.2 wicker, 1.25 suede and
// leather, 1.3 rope, 1.36 stainless steel, 1.45 the skirt's Nomex, 1.5 the
// envelope's ripstop nylon, 1.52 a seam between two rows of panels, 1.54 a
// load tape, 2 the burner flame. Instance ext.x: the envelope's scale (the
// passenger balloon's is the sport one made bigger), so the weave keeps its
// real size. The patterns fade out where a pixel covers several threads or
// strands, leaving their average colour.

/// 0..1: how much of a pattern of period `p` (m) is worth drawing, where a
/// pixel spans `fw` metres.
fn detail(fw: f32, p: f32) -> f32 { return 1.0 - smoothstep(0.25, 0.6, fw / p); }

/// Thin lines every `p` metres along x, `w` wide (a fraction of p), antialiased.
fn lines(x: f32, p: f32, w: f32, fw: f32) -> f32 {
  let d = abs(fract(x / p + 0.5) - 0.5) * p;
  return 1.0 - smoothstep(w * p * 0.5, w * p * 0.5 + fw, d);
}

fn balloon_surface(in: VOut, N0: vec3f, V: vec3f, m: f32, l: Light, sh: f32, px_world: f32) -> vec4f {
  let k = max(in.ext.x, 1.0);
  // A pixel's footprint in the piece's own metres.
  let px = px_world / k;
  let Lr = in.L / k;
  let up = l.upn;
  var N = N0;
  var albedo = in.col.rgb;
  var gloss = 0.0;
  var spec_pow = 30.0;
  var mirror = 0.0;
  if (m > 1.9) {
    // The burner flame: bright whatever the light, brighter at night; a
    // little flicker along it.
    let night = 1.0 - smoothstep(-0.05, 0.12, dot(up, G.sun_dir.xyz));
    let fl = textureSampleLevel(t_noise, s_repeat, Lr * vec3f(1.5, 0.4, 1.5), 0.0).r;
    return vec4f(in.col.rgb * (4.0 + 6.0 * night) * (0.8 + 0.4 * fl), 0.0);
  }
  if (m > 1.47) {
    // Ripstop nylon, polyurethane coated: a fine grid of heavier threads,
    // the cloth puckered and creased (most near the mouth, where it is
    // pulled in), a satin sheen.
    let r = max(length(Lr.xz), 0.05);
    let th = atan2(Lr.z, Lr.x);
    let a = th * r;
    let fw = px;
    let rs = max(lines(a, 0.0065, 0.12, fw), lines(Lr.y, 0.0065, 0.12, fw)) * detail(fw, 0.0065);
    albedo *= 1.0 - 0.10 * rs;
    // Creases: running mostly up and down the gores, stronger low down.
    let q = vec3f(Lr.x * 0.9, Lr.y * 0.12, Lr.z * 0.9);
    let e = 0.15;
    let n0 = textureSampleLevel(t_noise, s_repeat, q * 0.5, 0.0).g;
    let nx = textureSampleLevel(t_noise, s_repeat, (q + vec3f(e, 0.0, 0.0)) * 0.5, 0.0).g;
    let nz = textureSampleLevel(t_noise, s_repeat, (q + vec3f(0.0, 0.0, e)) * 0.5, 0.0).g;
    let ny = textureSampleLevel(t_noise, s_repeat, (q + vec3f(0.0, e, 0.0)) * 0.5, 0.0).g;
    let crease = (0.35 + 0.9 * (1.0 - smoothstep(0.5, 4.0, Lr.y))) * detail(px, 0.6);
    let g = vec3f(nx - n0, (ny - n0) * 0.3, nz - n0) / e;
    N = normalize(N - g * 0.035 * crease);
    // (A satin sheen, broad and soft: the creases barely move it.)
    let Ns = N0;
    gloss = 0.018;
    spec_pow = 10.0;
    // Older cloth: a little sooted and heat-faded at the mouth, sun-faded
    // at the crown, in blotches.
    let blot = textureSampleLevel(t_noise, s_repeat, Lr * 0.045 + vec3f(0.31, 0.07, 0.53), 0.0).b;
    albedo *= mix(0.72 + 0.12 * blot, 1.0, smoothstep(0.4, 3.5, Lr.y));
    albedo = mix(albedo, vec3f(luma(albedo)), 0.18 * smoothstep(15.0, 19.0, Lr.y) * (0.6 + 0.8 * blot));
    if (m > 1.53) {
      // A load tape: woven webbing sewn down both edges.
      let gw = 6.2831853 * r / 24.0;
      let gf = th / 6.2831853 * 24.0;
      let d = (gf - round(gf)) * gw;
      let stitch = lines(abs(d), 0.017, 0.12, px) * step(0.5, fract(Lr.y / 0.0045)) * detail(px, 0.0045);
      let weave = lines(Lr.y + d, 0.003, 0.5, px) * detail(px, 0.003);
      albedo *= (1.0 - 0.18 * weave) * (1.0 - 0.35 * stitch);
      gloss = 0.06;
    } else if (m > 1.51) {
      // A seam: two rows of stitches through the overlap, a fold between.
      let dy = fract(Lr.y / 0.02) - 0.5;
      let stitch = step(0.5, fract(a / 0.0045)) * detail(fw, 0.0045);
      albedo *= 1.0 - 0.25 * stitch * (1.0 - smoothstep(0.08, 0.14, abs(abs(dy) - 0.25)));
      albedo *= 0.9;
    }
    // The sun through the cloth: the side away from it glows in its own colours.
    let back = max(dot(-N, G.sun_dir.xyz), 0.0) * smoothstep(-0.05, 0.1, dot(up, G.sun_dir.xyz));
    let Hh = normalize(G.sun_dir.xyz - V);
    let spec = pow(max(dot(Ns, Hh), 0.0), spec_pow) * gloss * mix(1.0, sh, smoothstep(0.1, 0.4, dot(N0, G.sun_dir.xyz)));
    let ndv = max(dot(-V, Ns), 0.0);
    let fres = 0.03 + 0.2 * pow(1.0 - ndv, 5.0);
    // (Where the sun grazes the envelope the shadow map breaks up into acne;
    // a convex envelope cannot shadow itself there anyway.)
    let shf = mix(1.0, sh, smoothstep(0.1, 0.4, dot(N0, G.sun_dir.xyz)));
    var c = shade_lit(in.P, N, albedo, l, shf);
    // (Not this face's shadow: the face is turned from the sun, and the
    // shadow map's speckle would show through.)
    c += in.col.rgb * l.sun * back * 0.3;
    c += l.sun * spec + sky_lut(normalize(reflect(V, Ns) + up * 0.1)) * fres * 0.15;
    // (A taut, convex envelope has no nooks for the screen-space ambient
    // occlusion to darken: left out of it, as its noise showed on the
    // shaded side.)
    g_amb = vec3f(0.0);
    return vec4f(c, 0.0);
  }
  if (m > 1.42) {
    // The skirt: heavy Nomex, a coarse weave, sooted towards its hem.
    let fw = px;
    let w = max(lines(Lr.y, 0.004, 0.3, fw), lines(atan2(Lr.z, Lr.x) * length(Lr.xz), 0.004, 0.3, fw)) * detail(fw, 0.004);
    let soot = smoothstep(-0.5, -1.3, Lr.y) * (0.5 + 0.5 * textureSampleLevel(t_noise, s_repeat, Lr * 0.8, 0.0).r);
    albedo *= (1.0 - 0.15 * w) * (1.0 - 0.5 * soot);
    return vec4f(shade_lit(in.P, N, albedo, l, sh), 0.0);
  }
  if (m > 1.33) {
    // Stainless steel: the burner coils and the frame, mirror-bright where
    // the sky shows in them.
    let n = textureSampleLevel(t_noise, s_repeat, Lr * vec3f(30.0, 2.0, 30.0), 0.0).r;
    let R = reflect(V, N);
    let ndv = max(dot(-V, N), 0.0);
    let fres = 0.55 + 0.45 * pow(1.0 - ndv, 5.0);
    let Hh = normalize(G.sun_dir.xyz - V);
    var c = shade_lit(in.P, N, albedo * 0.35, l, sh);
    c += sky_lut(normalize(R + up * 0.1)) * albedo * fres * (0.85 + 0.3 * n);
    c += l.sun * pow(max(dot(N, Hh), 0.0), 90.0) * 2.0 * sh;
    return vec4f(c, 0.35);
  }
  if (m > 1.28) {
    // Rope: braided strands of polyester, fibres along them.
    let n = textureSampleLevel(t_noise, s_repeat, Lr * 18.0, 0.0);
    albedo *= 0.8 + 0.35 * n.r;
    return vec4f(shade_lit(in.P, N, albedo, l, sh), 0.0);
  }
  if (m > 1.23) {
    // Suede and leather: a soft nap, worn paler where hands go.
    let fw = px;
    let n = textureSampleLevel(t_noise, s_repeat, Lr * 9.0, 0.0);
    let n2 = textureSampleLevel(t_noise, s_repeat, Lr * 40.0, 0.0);
    albedo *= (0.85 + 0.3 * n.g) * (1.0 - 0.12 * n2.b * detail(fw, 0.02));
    let Hh = normalize(G.sun_dir.xyz - V);
    var c = shade_lit(in.P, N, albedo, l, sh);
    c += l.sun * pow(max(dot(N, Hh), 0.0), 12.0) * 0.04 * sh;
    return vec4f(c, 0.0);
  }
  // Wicker: rows of rattan woven in and out of upright stakes, each row
  // shifted one stake from the last; strands of slightly different colour,
  // rounded (lit on top, shadowed beneath), dark gaps between them.
  let Ln = normalize(in.Ln);
  var u = Lr.x;
  if (abs(Ln.x) > abs(Ln.z)) { u = Lr.z; }
  if (abs(Ln.y) > 0.7) { u = Lr.x + Lr.z * 0.7; }
  let stake = 0.034;
  let rowh = 0.0115;
  let sv = Lr.y / rowh;
  let ri = floor(sv);
  let fv = fract(sv);
  let shift = 0.5 * (ri - 2.0 * floor(ri * 0.5));
  let su = u / stake + shift;
  let si = floor(su);
  let fu = fract(su);
  let under = (ri + si) - 2.0 * floor((ri + si) * 0.5);
  let fw = px;
  let dt = detail(fw, rowh * 1.2);
  let tone = 0.74 + 0.42 * hash1(ri * 1.37 + floor(u / 0.9) * 7.1);
  let fibre = textureSampleLevel(t_noise, s_repeat, vec3f(u * 6.0, Lr.y * 90.0, 0.37), 0.0).r;
  var strand = in.col.rgb * tone * (0.85 + 0.3 * fibre);
  // Where the row passes behind a stake, the stake shows (a little darker).
  let stake_m = under * (1.0 - smoothstep(0.18, 0.32, abs(fu - 0.5)));
  strand = mix(strand, in.col.rgb * 0.72, stake_m);
  let gap = smoothstep(0.0, 0.2, fv) * smoothstep(1.0, 0.8, fv);
  strand *= 0.35 + 0.65 * gap;
  albedo = mix(in.col.rgb * 0.8, strand, dt);
  // Rounded strands: the normal tips up on their top half, down below.
  N = normalize(N + up * cos(3.14159 * fv) * 0.55 * dt * (1.0 - stake_m));
  return vec4f(shade_lit(in.P, N, albedo, l, sh), 0.0);
}
