-- Pit Box Shot: renders grid thumbnails of protected cars from inside the game
-- (docs/SPEC-grille.md GRILLE§7.1, findings in docs/csp-shot-research.md).
--
-- A protected KN5 only decodes inside Assetto Corsa once CSP is loaded, so the
-- game is the only renderer that can photograph those cars. Pit Box links this
-- app into `apps/lua` for one session, writes a job file, starts a practice
-- session through Content Manager, and this script renders every car of the
-- job off screen with `ac.GeometryShot` — its own camera and canvas: no HUD,
-- no scenery, transparent background —, writes the PNGs and a `done.json`,
-- then closes the game.
--
-- Without a job file this script does nothing: it is loaded with the game
-- (`LAZY = NONE`) because nobody opens its window.

local JOB_DIR = ac.getFolder(ac.FolderID.ACDocuments) .. '/pitbox_shot'
local JOB_FILE = JOB_DIR .. '/job.json'
local DONE_FILE = JOB_DIR .. '/done.json'

local job = nil
local done = { csp = ac.getPatchVersionCode(), cars = {} }
local status = 'idle'

local function saveDone()
  if not io.save(DONE_FILE, JSON.stringify(done), true) then
    ac.warn('pitbox_shot: could not write ' .. DONE_FILE)
  end
end

do
  local raw = io.load(JOB_FILE)
  if raw then
    local ok, parsed = pcall(JSON.parse, raw)
    if ok and type(parsed) == 'table' and type(parsed.cars) == 'table' then
      job = parsed
      status = 'waiting'
    else
      ac.warn('pitbox_shot: unreadable job file')
    end
  end
end

-- ---------------------------------------------------------------------------
-- Framing: the formulas of `src/lib/gridthumbs/framing.ts`, in the car's own
-- model space (the KN5 axes, which the Pit Box converter keeps), so the two
-- renderers frame a car identically. Checked by eye on two cars: same angle,
-- same size, same place in the frame.

local function rad(deg) return deg * math.pi / 180 end

local function corners(bmin, bmax)
  local list = {}
  for _, x in ipairs({ bmin.x, bmax.x }) do
    for _, y in ipairs({ bmin.y, bmax.y }) do
      for _, z in ipairs({ bmin.z, bmax.z }) do
        list[#list + 1] = vec3(x, y, z)
      end
    end
  end
  return list
end

-- framing.ts `TRUE_SCALE_RADIUS`, `TRUE_SCALE_MAX_SHRINK`, `GAME_FRAMING_RANGE`.
local TRUE_SCALE_RADIUS = 3.1
local TRUE_SCALE_MAX_SHRINK = 2.5
local GAME_FRAMING_RANGE = 3

-- The camera Kunos shot its own previews with (framing.ts `GAME_CAMERA_*`).
local GAME_CAMERA_POSITION = vec3(-3.867643, 1.42359, 4.70381)
local GAME_CAMERA_TARGET = vec3(0, 0.7, 0.5)
local GAME_CAMERA_FOV = 30

-- Whether the game's camera suits a car of this size (framing.ts
-- `gameFramingFits`): a mod built in the wrong unit gets the orbit instead.
local function gameFramingFits(radius)
  local ratio = radius / TRUE_SCALE_RADIUS
  return ratio > 1 / GAME_FRAMING_RANGE and ratio < GAME_FRAMING_RANGE
end

-- Camera for one template, in model space: position, target, field of view,
-- distance, and whether the lens shift applies (not under the game's camera,
-- where the place of the car in the frame is the point).
local function placeCamera(bmin, bmax, t, aspect)
  local radius = (bmax - bmin):length() / 2
  if (t.gameFraming or 0) > 0 then
    if gameFramingFits(radius) then
      local distance = (GAME_CAMERA_POSITION - GAME_CAMERA_TARGET):length()
      return GAME_CAMERA_POSITION:clone(), GAME_CAMERA_TARGET:clone(), GAME_CAMERA_FOV, distance, false
    end
  end

  local azimuth, elevation = rad(t.azimuth), rad(t.elevation)
  -- Direction from the target towards the camera.
  local dir = vec3(math.cos(elevation) * math.sin(azimuth), math.sin(elevation),
    math.cos(elevation) * math.cos(azimuth)):normalize()
  local right = vec3(0, 1, 0):cross(dir):normalize()
  local up = dir:clone():cross(right):normalize()

  local margin = 1 + t.margin / 100
  local tanV = math.tan(rad(t.fov) / 2) / margin
  local tanH = tanV * aspect

  local target = (bmin + bmax) / 2
  target.y = target.y + radius * t.height / 100

  local distance = 0
  for _, c in ipairs(corners(bmin, bmax)) do
    local d = c - target
    local depth = d:dot(dir)
    distance = math.max(distance, depth + math.abs(d:dot(right)) / tanH, depth + math.abs(d:dot(up)) / tanV)
  end
  if (t.trueScale or 0) > 0 then
    distance = math.min(math.max(TRUE_SCALE_RADIUS / tanH, distance), distance * TRUE_SCALE_MAX_SHRINK)
  end
  return target + dir * distance, target, t.fov, distance, true
end

-- ---------------------------------------------------------------------------
-- Lens shift: centres the projected box horizontally, as `centreHorizontally`
-- does in framing.ts. GeometryShot takes raw view/projection matrices, so the
-- engine's own matrices are read back and the projection is skewed. Which
-- entry carries the shift depends on the convention, so it is probed (it was
-- `row3.x` on CSP 0.3.2).

local function projectedCentre(view, proj, pts)
  local lo, hi = math.huge, -math.huge
  for _, p in ipairs(pts) do
    local c = proj:transform(view:transform(vec4(p.x, p.y, p.z, 1)))
    local x = c.x / c.w
    lo, hi = math.min(lo, x), math.max(hi, x)
  end
  return (lo + hi) / 2
end

-- Returns the shifted projection, and the shift itself (entry and amount) so
-- the floor's mirror pass can apply its mirror image.
local function shiftedProjection(view, proj, worldPts)
  local c0 = projectedCentre(view, proj, worldPts)
  for _, entry in ipairs({ { 'row3', 'x' }, { 'row1', 'z' } }) do
    local probe = proj:clone()
    probe[entry[1]][entry[2]] = probe[entry[1]][entry[2]] + 0.1
    local slope = (projectedCentre(view, probe, worldPts) - c0) / 0.1
    if math.abs(slope) > 0.5 then
      local fixed = proj:clone()
      fixed[entry[1]][entry[2]] = fixed[entry[1]][entry[2]] - c0 / slope
      return fixed, { entry = entry, amount = -c0 / slope }
    end
  end
  return proj, nil
end

-- ---------------------------------------------------------------------------
-- Colour pipeline. Each shot is rendered in HDR (float texture, linear light)
-- and finished here, in the game, by a shader:
--
-- 1. three fixed lamps are added. The game lets no light reach a
--    GeometryShot (`ac.LightSource` had no effect under either lighting), so
--    the light Pit Box's studio gives a car has to be put back by hand. They
--    were the template's lamps until those left Pit Box's own render
--    (2026-11-12); they stay here at the values every preset was calibrated
--    with, since without them the shot has only the game's ambient. Two extra
--    shots from the same camera give the surface colour
--    (`SampleColor`, used as handed over: decoding it from sRGB made orange
--    too red) and the world normal (`SampleNormal`, encoded n·0.5+0.5); each
--    lamp adds `albedo × max(0, n·L)` and a Blinn-Phong highlight;
-- 2. the same tone mapping as Pit Box's renderer (three.js
--    `NeutralToneMapping`, the Khronos PBR Neutral curve) and an sRGB encode —
--    without them the PNG held raw HDR values clipped at 1;
-- 3. the template's colour grade (look.ts `gradeColours`: cool tint, then
--    saturation, on the encoded values).
--
-- The 2× supersampled shot is averaged down by the bilinear sample at the
-- half-resolution pixel centres. Edges are premultiplied (blended over a
-- transparent black clear) and unpremultiplied before anything else.

local FINISH = [[
float3 neutral(float3 color) {
  const float startCompression = 0.8 - 0.04;
  const float desaturation = 0.15;
  float x = min(color.r, min(color.g, color.b));
  float offset = x < 0.08 ? x - 6.25 * x * x : 0.04;
  color -= offset;
  float peak = max(color.r, max(color.g, color.b));
  if (peak < startCompression) return color;
  const float d = 1 - startCompression;
  float newPeak = 1 - d * d / (peak + d - startCompression);
  color *= newPeak / peak;
  float g = 1 - 1 / (desaturation * (peak - newPeak) + 1);
  return lerp(color, newPeak, g);
}
float3 toSrgb(float3 c) {
  c = saturate(c);
  return c <= 0.0031308 ? c * 12.92 : 1.055 * pow(c, 1 / 2.4) - 0.055;
}
float lamp(float3 n, float4 l) { return l.w * saturate(dot(n, l.xyz)); }
float shine(float3 n, float3 v, float4 l) { return l.w * pow(saturate(dot(n, normalize(l.xyz + v))), gGloss); }
// Tone mapping only: the colour grade comes last, on the whole image, as
// Pit Box grades its finished canvas (background included).
float3 finish(float3 rgb) {
  return saturate(toSrgb(neutral(rgb * gExposure)));
}
float3 grade(float3 c) {
  c *= 1 + gTint;
  float luma = dot(c, float3(0.2126, 0.7152, 0.0722));
  return saturate(luma + (c - luma) * gSaturation);
}
float4 over(float4 top, float4 below) {
  return float4(top.rgb + below.rgb * (1 - top.a), top.a + below.a * (1 - top.a));
}
// The baked background (`backdropTexture`): the card's mat, an ellipse from
// `gMatHi` at 44 % of the height to `gMatLo`, interpolated in screen colours
// like a canvas gradient. Not exposed, not tone mapped: exactly the mat.
float3 backdrop(float2 uv) {
  float2 p = float2((uv.x - 0.5) * 256 / (256.0 / 144.0), uv.y * 144 - 144 * 0.44);
  return lerp(gMatHi, gMatLo, saturate(length(p) / (144 * 0.76)));
}
// The floor of a preset that has one (GRILLE§7.1), as the Pit Box rig lays it
// (floor.ts, studioFloor.ts, floorMirror.ts), premultiplied, in screen
// colours, from the bottom up: the studio ground (Officiel), the reflection,
// the pool — the wide glow with its contact darkening, or the disc of light.
float4 floorLayer(float2 uv) {
  float3 far = lerp(lerp(gCornerTL, gCornerTR, uv.x), lerp(gCornerBL, gCornerBR, uv.x), uv.y);
  float3 dir = far - gCamera;
  float facing = dot(dir, gFloorUp);
  if (facing >= 0) return 0;
  float3 hit = gCamera + dir * (dot(gFloorCentre - gCamera, gFloorUp) / facing);
  float r = length(hit - gFloorCentre) / gFloorHalf;
  float rp = length(hit - gPoolCentre) / gPoolHalf;
  float4 acc = 0;
  if (gGround > 0) {
    // Solid to half its radius, then fading to nothing (`groundTexture`).
    float rg = length(hit - gPoolCentre) / (gPoolHalf * 3);
    float a = rg < 0.5 ? 1 : saturate(1 - (rg - 0.5) / 0.5);
    acc = over(float4(gGroundColor * a, a), acc);
  }
  if (gReflection > 0 && r < 1) {
    // The mirror camera's image is the reflection flipped left to right.
    float2 m = float2(1 - uv.x, uv.y);
    float4 sum = 0;
    float total = 0;
    for (int i = -2; i <= 2; i++) {
      for (int j = -2; j <= 2; j++) {
        float w = max(1 - length(float2(i, j)) / 3.2, 0);
        sum += txMirror.SampleLevel(samLinearClamp, m + float2(i, j) * gReflectionBlur, 0) * w;
        total += w;
      }
    }
    float4 mir = sum / total;
    if (mir.a > 0.001) {
      float fade = 1 - smoothstep(0.75 * 0.35, 0.75, r);
      float a = mir.a * gReflection * fade;
      acc = over(float4(finish(max(mir.rgb / mir.a, 0) * gMirrorTint) * a, a), acc);
    }
  }
  if (rp < 1) {
    float pa;
    if (gDisc > 0) {
      // The disc of light: 0.11 at the centre, 0.09 where its edge starts,
      // nothing at the rim (`discTexture`).
      pa = rp < 1 - gDiscFade ? lerp(0.11, 0.09, rp / (1 - gDiscFade)) : lerp(0.09, 0, (rp - (1 - gDiscFade)) / gDiscFade);
    } else {
      // The glow: 0.11 at the centre, 0.066 at 45 %, 0.018 at 75 %, 0 at the rim.
      pa = rp < 0.45 ? lerp(0.11, 0.066, rp / 0.45)
        : rp < 0.75 ? lerp(0.066, 0.018, (rp - 0.45) / 0.3) : lerp(0.018, 0, (rp - 0.75) / 0.25);
    }
    pa *= gFloorOpacity;
    acc = over(float4((gFloorBright * pa).xxx, pa), acc);
    if (gDisc <= 0) {
      // Contact darkening, the last centimetre under the body: 0.3 to 0 over
      // a third of the radius (`poolTexture`).
      float ca = rp < 0.34 ? 0.3 * (1 - rp / 0.34) * gFloorOpacity : 0;
      acc = over(float4(0, 0, 0, ca), acc);
    }
  }
  return acc;
}
float4 main(PS_IN pin) {
  float4 below = gBackground > 0 ? float4(backdrop(pin.Tex), 1) : 0;
  if (gFloor > 0) below = over(floorLayer(pin.Tex), below);
  float4 c = txShot.SampleLevel(samLinearClamp, pin.Tex, 0);
  float4 outc = below;
  if (c.a > 0) {
    float3 rgb = max(c.rgb / c.a, 0) * gAmbient;
    float4 alb = txAlbedo.SampleLevel(samLinearClamp, pin.Tex, 0);
    float4 nrm = txNormal.SampleLevel(samLinearClamp, pin.Tex, 0);
    if (nrm.a > 0.5) {
      float3 n = normalize(nrm.xyz / nrm.a * 2 - 1);
      float3 albedo = max(alb.rgb / max(alb.a, 0.0001), 0);
      float3 v = normalize(gView);
      rgb += albedo * (lamp(n, gKey) + lamp(n, gFill) + lamp(n, gRim)) * gLight;
      rgb += (shine(n, v, gKey) + shine(n, v, gFill) + shine(n, v, gRim)) * gSpecular;
    }
    float a = saturate(c.a);
    outc = over(float4(finish(rgb) * a, a), below);
  }
  if (outc.a <= 0) return 0;
  return float4(grade(outc.rgb / outc.a), outc.a);
}
]]

-- Calibrated on a Supra and a Ferrari against their Pit Box thumbnails
-- (csp-shot-research.md, runs 5 to 7): lamps at 2.0 for a key at 100 %,
-- exposure 0.8, highlights 0.3 with a glossiness of 60.
local LIGHT_AT_100 = 2.0
local EXPOSURE = 0.8
local SPECULAR = 0.3
local GLOSS = 60
-- look.ts `GRADE_TINT`, the game's own cast at 100 % cool tint.
local GRADE_TINT = vec3(-0.03, 0.02, 0.055)

-- The fixed lamps, as Pit Box's studio placed its own before they were
-- removed: offset from the camera azimuth and elevation, in degrees; fill and
-- rim as a share of the key (25 % and 60 %, the old default).
local function lampDirections(state)
  local base = math.atan2(state.camDir.x, state.camDir.z)
  local function at(offset, elevation, share)
    local a, e = base + rad(offset), rad(elevation)
    local p = state.center + vec3(math.cos(e) * math.sin(a), math.sin(e), math.cos(e) * math.cos(a)) * (state.radius * 4)
    local d = (state.modelToWorld:transformPoint(p) - state.centerWorld):normalize()
    return vec4(d.x, d.y, d.z, share)
  end
  return at(45, 35, 1), at(-135, 20, 0.25), at(175, 12, 0.6)
end

-- Kept so they are not tried again: the default neutral lighting burns
-- highlights and tints silver blue; `setBestSceneShotQuality` lights the car
-- with the track's sun and ignores the ambient colour; YEBIS composites the
-- blurred track behind the car and loses the transparency; the reflection
-- gradient washes paint out to pastel. The showroom cubemap (`'menu'`) gives
-- believable colours. A Pit Box-like environment posed by the shader was
-- tried too (run 9): no clear winner, and chrome went black.
local function showroom(s)
  s:setShadersType(render.ShadersType.Main):setMaxLayer(5)
  s:setAmbientColor(rgbm(1, 1, 1, 0.5))
  s:setReflectionColor(rgbm(0.35, 0.35, 0.35, 1), rgbm(0.08, 0.08, 0.08, 1))
  s:overrideReflectionCubemap('menu')
end

local PASSES = {
  { id = 'base', setup = showroom },
  -- Opaque only: glass would hide the cabin from the relighting.
  { id = 'normal', opaque = true, setup = function(s) s:setShadersType(render.ShadersType.SampleNormal) end },
  { id = 'albedo', opaque = true, setup = function(s) s:setShadersType(render.ShadersType.SampleColor) end },
}

local function renderPass(state, view, pass)
  local shot = ac.GeometryShot(state.root, vec2(job.width * job.supersample, job.height * job.supersample), 1, false,
    render.AntialiasingMode.None, render.TextureFormat.R16G16B16A16.Float)
  shot:setClearColor(rgbm(0, 0, 0, 0)):setOpaqueAlphaFix(true):setTransparentPass(not pass.opaque)
  shot:setClippingPlanes(view.near, view.far)
  -- The contact shadow every car ships (`body_shadow.png`): without it the
  -- car floats. Only on the colour pass, which carries the alpha.
  if pass.id == 'base' and view.shadow then shot:setFakeCarShadows(true) end
  pass.setup(shot)
  if view.view then
    shot:update(view.view, view.proj, view.pos)
  else
    shot:update(view.pos, view.look, view.up, view.fov)
    view.view = shot:viewMatrix():clone()
    view.proj = shot:projectionMatrix():clone()
    if view.mirrorOf and view.mirrorOf.shift then
      -- The reflection's lens shift is the main one, mirrored: the image is
      -- flipped back left to right when it is laid on the floor.
      local e = view.mirrorOf.shift.entry
      view.proj[e[1]][e[2]] = view.proj[e[1]][e[2]] - view.mirrorOf.shift.amount
    elseif view.shift then
      view.proj, view.shift = shiftedProjection(view.view, view.proj, state.worldCorners)
    end
    shot:update(view.view, view.proj, view.pos)
  end
  return shot
end

-- Points on the rays through the four corners of the frame (top-left,
-- top-right, bottom-left, bottom-right), in world space: the shader
-- interpolates them to find where each pixel meets the floor. Any depth will
-- do as long as it is the same for all four; the lens shift is included,
-- being part of the projection.
local function screenCorners(view)
  local invProj, invView = view.proj:inverse(), view.view:inverse()
  local function corner(x, y)
    local p = invProj:transform(vec4(x, y, 0.5, 1))
    local w = invView:transform(vec4(p.x / p.w, p.y / p.w, p.z / p.w, 1))
    return vec3(w.x, w.y, w.z)
  end
  return { corner(-1, 1), corner(1, 1), corner(-1, -1), corner(1, -1) }
end

-- Pit Box's mirror tints its reflection with three.js `Reflector`'s default
-- colour, 0x7F7F7F: 0.212 once converted to linear light.
local MIRROR_TINT = 0.212

-- look.ts `EXPOSURE_TARGET`/`EXPOSURE_MIN`/`EXPOSURE_MAX`.
local EXPOSURE_TARGET = 0.015
local EXPOSURE_MIN = 0.7
local EXPOSURE_MAX = 1.8
-- floor.ts `GROUND_SHADE`: the studio floor around the disc, against the
-- backdrop's darker colour.
local GROUND_SHADE = 0.4

local function hexColour(hex, fallback)
  local r, g, b = tostring(hex or ''):match('^#(%x%x)(%x%x)(%x%x)$')
  if not r then return fallback end
  return vec3(tonumber(r, 16) / 255, tonumber(g, 16) / 255, tonumber(b, 16) / 255)
end
local function toLinear(c)
  local function one(v) return v <= 0.04045 and v / 12.92 or math.pow((v + 0.055) / 1.055, 2.4) end
  return vec3(one(c.x), one(c.y), one(c.z))
end
local function toScreen(c)
  local function one(v) return v <= 0.0031308 and v * 12.92 or 1.055 * math.pow(v, 1 / 2.4) - 0.055 end
  return vec3(one(c.x), one(c.y), one(c.z))
end

local function shaderTextures(shots)
  return { txShot = shots.base, txAlbedo = shots.albedo, txNormal = shots.normal, txMirror = shots.mirror or false }
end

-- What the finishing shader needs for one template. `exposure` multiplies the
-- base exposure (auto exposure); `metering` leaves the colour grade out, as
-- Pit Box meters its render before grading it.
local function shaderValues(state, view, t, exposure, metering)
  local key, fill, rim = lampDirections(state)
  local corners = screenCorners(view)
  local floor = (t.floor or 0) > 0
  -- The pool: the wide glow over six car radii around the car, or the disc of
  -- light (`poolSize`), centred on the car's own origin and sized from the
  -- reference radius under the game's camera, as in the game (floor.ts
  -- `placePool`).
  local disc = (t.poolSize or 0) > 0
  local poolCentre, poolHalf = state.floorCentre, state.radius * 3
  if disc then
    local studio = (t.gameFraming or 0) > 0 and gameFramingFits(state.radius)
    poolHalf = (studio and TRUE_SCALE_RADIUS or state.radius) * t.poolSize / 100
    if studio then poolCentre = state.modelToWorld:transformPoint(vec3(0, state.bmin.y, 0)) end
  end
  local hi = hexColour(t.matHi, vec3(0.1, 0.1, 0.11))
  local lo = hexColour(t.matLo, vec3(0.05, 0.05, 0.06))
  return {
    gExposure = EXPOSURE * exposure, gAmbient = (t.ambient or 100) / 100,
    gLight = LIGHT_AT_100, gSpecular = SPECULAR, gGloss = GLOSS,
    gKey = key, gFill = fill, gRim = rim, gView = view.look * -1,
    gTint = metering and vec3(0, 0, 0) or GRADE_TINT * ((t.coolTint or 0) / 100),
    gSaturation = metering and 1 or (t.saturation or 100) / 100,
    gBackground = (t.background or 0) > 0 and 1 or 0, gMatHi = hi, gMatLo = lo,
    gFloor = floor and 1 or 0,
    gFloorOpacity = math.min(1, (t.floor or 0) / 100), gFloorBright = math.max(1, (t.floor or 0) / 100),
    gReflection = view.mirrored and (t.reflection or 0) / 100 or 0,
    gReflectionBlur = ((t.reflectionBlur or 0) / 10) * 0.002, gMirrorTint = MIRROR_TINT,
    gCamera = view.pos, gFloorUp = state.floorUp, gFloorCentre = state.floorCentre,
    gFloorHalf = state.radius * 3,
    gPoolCentre = poolCentre, gPoolHalf = poolHalf,
    gDisc = disc and 1 or 0, gDiscFade = math.max(0.02, (t.poolEdge or 30) / 100),
    -- The studio floor around the disc, only with a baked background.
    gGround = (disc and floor and (t.background or 0) > 0) and 1 or 0,
    gGroundColor = toScreen(toLinear(lo) * GROUND_SHADE),
    gCornerTL = corners[1], gCornerTR = corners[2], gCornerBL = corners[3], gCornerBR = corners[4],
  }
end

local function carIndexOf(id)
  for i = 0, ac.getSim().carsCount - 1 do
    if ac.getCarID(i) == id then return i end
  end
  return nil
end

-- Builds the steps for one car. Each step runs on its own frame, so nothing
-- reads a texture rendered on the same frame.
local function planCar(entry, steps)
  local record = { id = entry.id, outputs = {} }
  done.cars[#done.cars + 1] = record
  local state = {}

  steps[#steps + 1] = function()
    local index = carIndexOf(entry.id)
    if not index then
      record.error = 'not_in_session'
      return
    end
    local car = ac.getCar(index)
    state.index = index
    -- A car seen from afar is drawn with a lighter model (LOD): on a grid,
    -- only the player's car is close to the main camera.
    ac.forceDetailedCarLOD(index)
    -- The cars lined up as opponents have their driver seated, the player's
    -- standing in its pit box has none: no driver at all, as in the Pit Box
    -- renders, which draw the car alone.
    ac.setDriverVisible(index, false)
    -- A car standing in its pit box gets its driver door opened by the game.
    ac.setDriverDoorOpen(index, false, true)
    record.skin = ac.getCarSkinID(index)
    state.root = ac.findNodes('carRoot:' .. index)
    state.modelToWorld = state.root:getWorldTransformationRaw():clone()
    -- The game's own car box (LOD D or collider, model space): the mesh box
    -- of the car root reaches a metre below the ground.
    state.bmin, state.bmax = car.aabbCenter - car.aabbSize / 2, car.aabbCenter + car.aabbSize / 2
    state.center = (state.bmin + state.bmax) / 2
    state.centerWorld = state.modelToWorld:transformPoint(state.center)
    state.radius = (state.bmax - state.bmin):length() / 2
    -- The floor plane: under the car's box, facing the car's up.
    local origin = state.modelToWorld:transformPoint(vec3(0, 0, 0))
    state.floorUp = (state.modelToWorld:transformPoint(vec3(0, 1, 0)) - origin):normalize()
    state.floorCentre = state.modelToWorld:transformPoint(vec3(state.center.x, state.bmin.y, state.center.z))
    state.worldCorners = {}
    for _, c in ipairs(corners(state.bmin, state.bmax)) do
      state.worldCorners[#state.worldCorners + 1] = state.modelToWorld:transformPoint(c)
    end
  end

  -- The detailed model and its textures need a few frames once forced:
  -- forced again on every one of them (the call holds "for a few frames").
  local settle = 0
  steps[#steps + 1] = function()
    if not state.index then return end
    ac.forceDetailedCarLOD(state.index)
    settle = settle + 1
    if settle < 30 then return 'again' end
    record.activeLOD = ac.getCar(state.index).activeLOD
  end

  for _, output in ipairs(entry.outputs) do
    local t = output.template
    local view, shots, canvas = nil, {}, nil

    steps[#steps + 1] = function()
      if not state.root then return end
      local camLocal, targetLocal, fov, distance, shift = placeCamera(state.bmin, state.bmax, t, job.width / job.height)
      local m = state.modelToWorld
      local pos, target = m:transformPoint(camLocal), m:transformPoint(targetLocal)
      view = {
        pos = pos, look = (target - pos):normalize(),
        up = (m:transformPoint(targetLocal + vec3(0, 1, 0)) - target):normalize(),
        fov = fov, near = math.max(distance / 100, 0.01), far = distance * 4,
        shift = shift, shadow = (t.shadow or 0) > 0,
      }
      state.camDir = camLocal - targetLocal
    end

    for _, pass in ipairs(PASSES) do
      steps[#steps + 1] = function()
        if not view then return end
        ac.forceDetailedCarLOD(state.index)
        local ok, err = pcall(function() shots[pass.id] = renderPass(state, view, pass) end)
        if not ok then record.outputs[output.file] = 'error: ' .. tostring(err) end
      end
    end

    -- The floor's reflection: the car seen from the camera mirrored under the
    -- floor plane (a planar mirror), as Pit Box's `Reflector` does.
    local withFloor = (t.floor or 0) > 0
    local withReflection = withFloor and (t.reflection or 0) > 0
    if withReflection then
      steps[#steps + 1] = function()
        if not view or not view.view then return end
        local n, f = state.floorUp, state.floorCentre
        local function point(p) return p - n * (2 * (p - f):dot(n)) end
        local function vector(v) return v - n * (2 * v:dot(n)) end
        local mirror = {
          pos = point(view.pos), look = vector(view.look), up = vector(view.up),
          fov = view.fov, near = view.near, far = view.far, mirrorOf = view,
        }
        local ok, err = pcall(function() shots.mirror = renderPass(state, mirror, PASSES[1]) end)
        if ok then view.mirrored = true else record.outputs[output.file] = 'error: ' .. tostring(err) end
      end
    end

    -- Auto exposure (look.ts `meteredExposure`): the whole frame rendered small
    -- at the base exposure, read back, its mean brightness in linear light
    -- giving the exposure of the final image. Read back asynchronously, hence
    -- a step that waits for it.
    local exposure = 1
    if (t.autoExposure or 0) > 0 then
      local meter, measured = nil, nil
      steps[#steps + 1] = function()
        if not (shots.base and shots.normal and shots.albedo) then return end
        meter = ui.ExtraCanvas(vec2(64, 36), 1, render.AntialiasingMode.None)
        meter:updateWithShader({ textures = shaderTextures(shots), values = shaderValues(state, view, t, 1, true),
          shader = FINISH })
      end
      steps[#steps + 1] = function()
        if not meter then
          measured = false
          return
        end
        meter:accessData(function(err, data)
          if err or not data then
            measured = false
            return
          end
          local sum = 0
          for y = 0, 35 do
            for x = 0, 63 do
              local c = data:color(x, y)
              local luma = c.r * 0.2126 + c.g * 0.7152 + c.b * 0.0722
              sum = sum + math.pow(luma, 2.2) * c.mult
            end
          end
          measured = sum / (64 * 36)
          data:dispose()
        end)
      end
      steps[#steps + 1] = function()
        if measured == nil then return 'again' end
        if meter then meter:dispose() end
        if measured and measured > 0 then
          exposure = math.min(EXPOSURE_MAX, math.max(EXPOSURE_MIN, EXPOSURE_TARGET / measured))
        elseif measured == 0 then
          exposure = EXPOSURE_MAX
        end
        record.exposure = exposure
      end
    end

    steps[#steps + 1] = function()
      if not (shots.base and shots.normal and shots.albedo) then return end
      canvas = ui.ExtraCanvas(vec2(job.width, job.height), 1, render.AntialiasingMode.None)
      local ok, err = pcall(function()
        canvas:updateWithShader({ textures = shaderTextures(shots), values = shaderValues(state, view, t, exposure, false),
          shader = FINISH })
      end)
      if not ok then
        record.outputs[output.file] = 'error: ' .. tostring(err)
        canvas:dispose()
        canvas = nil
      end
    end

    steps[#steps + 1] = function()
      if canvas then
        local ok, err = pcall(function() canvas:save(output.file, ac.ImageFormat.PNG) end)
        record.outputs[output.file] = ok and 'ok' or ('error: ' .. tostring(err))
        canvas:dispose()
      end
      for _, shot in pairs(shots) do shot:dispose() end
      shots = {}
      saveDone()
    end
  end
end

local steps = nil
local stepIndex = 1
local waited = 0
local quitAt = nil
local paused = false

local function update(dt)
  if status == 'idle' or status == 'finished' then
    if quitAt and os.preciseClock() >= quitAt then
      quitAt = nil
      ac.shutdownAssettoCorsa()
    end
    return
  end

  if status == 'waiting' then
    -- A race lines several cars up on the grid: the game is paused before the
    -- start, so nothing moves. Asked again until the game accepts it (not
    -- while still loading). No lighter models either, for any car.
    if not paused then paused = ac.tryToPause(true) end
    ac.overrideLODMultiplier(nil, math.huge)
    -- Textures and LOD A need a moment after the session starts.
    waited = waited + dt
    if waited < (job.warmup or 8) then return end
    steps = {}
    for _, entry in ipairs(job.cars) do planCar(entry, steps) end
    status = 'shooting'
    saveDone()
    return
  end

  if status == 'shooting' then
    local step = steps[stepIndex]
    if step then
      local ok, err = pcall(step)
      -- A step waiting for something (a read-back) runs again next frame.
      if ok and err == 'again' then return end
      if not ok then
        ac.error('pitbox_shot: ' .. tostring(err))
        done.lastError = tostring(err)
      end
      stepIndex = stepIndex + 1
      return
    end
    done.finished = true
    done.paused = paused
    status = 'finished'
    saveDone()
    if job.quit then quitAt = os.preciseClock() + 1 end
  end
end

-- An error out of a step is caught there; one here (planning, a broken job)
-- would stop the app frame after frame without a word anywhere — CSP does not
-- log an app's errors. It goes to a file next to the job, read by Pit Box's log
-- when the session produced nothing.
local failed = false
function script.update(dt)
  if failed then return end
  local ok, err = pcall(update, dt)
  if not ok then
    failed = true
    io.save(JOB_DIR .. '/error.txt', tostring(err))
    ac.error('pitbox_shot: ' .. tostring(err))
  end
end

function windowMain()
  ui.text('Pit Box Shot: ' .. status)
  if steps then ui.text(string.format('%d / %d', math.min(stepIndex - 1, #steps), #steps)) end
end
