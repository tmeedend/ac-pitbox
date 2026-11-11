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

-- Camera for one template, in model space: position, target, field of view,
-- distance, and whether the lens shift applies (not under the game's camera,
-- where the place of the car in the frame is the point).
local function placeCamera(bmin, bmax, t, aspect)
  local radius = (bmax - bmin):length() / 2
  if (t.gameFraming or 0) > 0 then
    local ratio = radius / TRUE_SCALE_RADIUS
    if ratio > 1 / GAME_FRAMING_RANGE and ratio < GAME_FRAMING_RANGE then
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

local function shiftedProjection(view, proj, worldPts)
  local c0 = projectedCentre(view, proj, worldPts)
  for _, entry in ipairs({ { 'row3', 'x' }, { 'row1', 'z' } }) do
    local probe = proj:clone()
    probe[entry[1]][entry[2]] = probe[entry[1]][entry[2]] + 0.1
    local slope = (projectedCentre(view, probe, worldPts) - c0) / 0.1
    if math.abs(slope) > 0.5 then
      local fixed = proj:clone()
      fixed[entry[1]][entry[2]] = fixed[entry[1]][entry[2]] - c0 / slope
      return fixed
    end
  end
  return proj
end

-- ---------------------------------------------------------------------------
-- Colour pipeline. Each shot is rendered in HDR (float texture, linear light)
-- and finished here, in the game, by a shader:
--
-- 1. the template's three lamps are added. The game lets no light reach a
--    GeometryShot (`ac.LightSource` had no effect under either lighting), so
--    two extra shots from the same camera give the surface colour
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
float4 main(PS_IN pin) {
  float4 c = txShot.SampleLevel(samLinearClamp, pin.Tex, 0);
  if (c.a <= 0) return 0;
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
  float3 outc = toSrgb(neutral(rgb * gExposure));
  outc *= 1 + gTint;
  float luma = dot(outc, float3(0.2126, 0.7152, 0.0722));
  outc = luma + (outc - luma) * gSaturation;
  return float4(saturate(outc), saturate(c.a));
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

-- The template's lamps (gridThumbs.svelte.ts `placeLights`): offset from the
-- camera azimuth and elevation, in degrees; fill and rim as a share of the key.
local function lampDirections(state, t)
  local base = math.atan2(state.camDir.x, state.camDir.z)
  local function at(offset, elevation, share)
    local a, e = base + rad(offset), rad(elevation)
    local p = state.center + vec3(math.cos(e) * math.sin(a), math.sin(e), math.cos(e) * math.cos(a)) * (state.radius * 4)
    local d = (state.modelToWorld:transformPoint(p) - state.centerWorld):normalize()
    return vec4(d.x, d.y, d.z, share)
  end
  return at(45, 35, 1), at(-135, 20, (t.fill or 25) / 100), at(175, 12, (t.rim or 60) / 100)
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
    if view.shift then view.proj = shiftedProjection(view.view, view.proj, state.worldCorners) end
    shot:update(view.view, view.proj, view.pos)
  end
  return shot
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
    state.worldCorners = {}
    for _, c in ipairs(corners(state.bmin, state.bmax)) do
      state.worldCorners[#state.worldCorners + 1] = state.modelToWorld:transformPoint(c)
    end
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
        local ok, err = pcall(function() shots[pass.id] = renderPass(state, view, pass) end)
        if not ok then record.outputs[output.file] = 'error: ' .. tostring(err) end
      end
    end

    steps[#steps + 1] = function()
      if not (shots.base and shots.normal and shots.albedo) then return end
      local key, fill, rim = lampDirections(state, t)
      canvas = ui.ExtraCanvas(vec2(job.width, job.height), 1, render.AntialiasingMode.None)
      local ok, err = pcall(function()
        canvas:updateWithShader({
          textures = { txShot = shots.base, txAlbedo = shots.albedo, txNormal = shots.normal },
          values = {
            gExposure = EXPOSURE, gAmbient = (t.ambient or 100) / 100,
            gLight = LIGHT_AT_100 * (t.key or 100) / 100, gSpecular = SPECULAR, gGloss = GLOSS,
            gKey = key, gFill = fill, gRim = rim, gView = view.look * -1,
            gTint = GRADE_TINT * ((t.coolTint or 0) / 100), gSaturation = (t.saturation or 100) / 100,
          },
          shader = FINISH,
        })
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

function script.update(dt)
  if status == 'idle' or status == 'finished' then
    if quitAt and os.preciseClock() >= quitAt then
      quitAt = nil
      ac.shutdownAssettoCorsa()
    end
    return
  end

  if status == 'waiting' then
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
      if not ok then
        ac.error('pitbox_shot: ' .. tostring(err))
        done.lastError = tostring(err)
      end
      stepIndex = stepIndex + 1
      return
    end
    done.finished = true
    status = 'finished'
    saveDone()
    if job.quit then quitAt = os.preciseClock() + 1 end
  end
end

function windowMain()
  ui.text('Pit Box Shot: ' .. status)
  if steps then ui.text(string.format('%d / %d', math.min(stepIndex - 1, #steps), #steps)) end
end
