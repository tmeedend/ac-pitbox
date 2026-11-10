-- Pit Box Shot: renders grid thumbnails of protected cars from inside the game.
--
-- A protected KN5 only decodes inside Assetto Corsa once CSP is loaded, so the
-- game is the only renderer that can photograph those cars. Pit Box writes a
-- job file, starts a session through Content Manager, and this script renders
-- every car of the job off screen with `ac.GeometryShot` (its own camera, its
-- own canvas: no HUD, no scenery, transparent background), writes the PNGs and
-- a `done.json` as it goes, then closes the game.
--
-- Proof of concept (lot 1): several rendering variants per car, so the
-- lighting question can be settled by looking at them side by side, and every
-- measurement the orchestrator will need later (bounding box, active LOD,
-- which folders accept a write) goes into `done.json`.
--
-- Without a job file this script does nothing: it is loaded with the game
-- (`LAZY = NONE`) because nobody opens its window.

local JOB_DIR = ac.getFolder(ac.FolderID.ACDocuments) .. '/pitbox_shot'
local JOB_FILE = JOB_DIR .. '/job.json'
local DONE_FILE = JOB_DIR .. '/done.json'

local job = nil
local done = { started = os.date('%Y-%m-%dT%H:%M:%S'), csp = ac.getPatchVersionCode(), cars = {}, probes = {} }
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
-- Framing: the same formula as `src/lib/gridthumbs/framing.ts` (`placeCamera`),
-- in the car's own model space, so the two renderers frame a car identically.

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

-- Returns camera position and target in model space.
local function fitOrbit(bmin, bmax, t, aspect)
  local azimuth, elevation = rad(t.azimuth), rad(t.elevation)
  -- Direction from the target towards the camera.
  local dir = vec3(math.cos(elevation) * math.sin(azimuth), math.sin(elevation),
    math.cos(elevation) * math.cos(azimuth)):normalize()
  local right = vec3(0, 1, 0):cross(dir):normalize()
  local up = dir:clone():cross(right):normalize()

  local margin = 1 + t.margin / 100
  local tanV = math.tan(rad(t.fov) / 2) / margin
  local tanH = tanV * aspect

  local center = (bmin + bmax) / 2
  local radius = (bmax - bmin):length() / 2
  local target = center:clone()
  target.y = target.y + radius * t.height / 100

  local distance = 0
  for _, c in ipairs(corners(bmin, bmax)) do
    local d = c - target
    local depth = d:dot(dir)
    distance = math.max(distance, depth + math.abs(d:dot(right)) / tanH, depth + math.abs(d:dot(up)) / tanV)
  end
  return target + dir * distance, target, distance
end

-- ---------------------------------------------------------------------------
-- Bounding box, in model space. `getLocalAABB` is documented as "local mesh
-- coordinates, node transforms taken into account": whether that includes the
-- car root's own world placement is not said, so both readings are recorded
-- and the one centred near the model origin wins (a car on a track never sits
-- within a few metres of the world origin).

--
-- Measured on the first run: the mesh box of the car root reaches 1.2 m
-- *below* the ground (something hidden under the car, likely a shadow or LOD
-- helper), which left the car small and high in the frame. The game's own car
-- box (`aabbCenter`/`aabbSize`, from LOD D or the collider, in model space)
-- matched the real S2000 to a few centimetres, so it is the default; the mesh
-- box is still recorded, and `job.box = 'mesh'` brings it back.

local function measureBox(root, worldToModel, car)
  local bmin, bmax, meshes = root:getLocalAABB()
  local info = { meshes = meshes, raw = { min = { bmin.x, bmin.y, bmin.z }, max = { bmax.x, bmax.y, bmax.z } },
    physicsCenter = { car.aabbCenter.x, car.aabbCenter.y, car.aabbCenter.z },
    physicsSize = { car.aabbSize.x, car.aabbSize.y, car.aabbSize.z } }
  if job.box ~= 'mesh' then
    info.space = 'car'
    return car.aabbCenter - car.aabbSize / 2, car.aabbCenter + car.aabbSize / 2, info
  end
  if ((bmin + bmax) / 2):length() < 5 then
    info.space = 'model'
    return bmin, bmax, info
  end
  -- World-space box: bring its corners back into model space.
  local lo, hi = vec3(math.huge, math.huge, math.huge), vec3(-math.huge, -math.huge, -math.huge)
  for _, c in ipairs(corners(bmin, bmax)) do
    local m = worldToModel:transformPoint(c)
    lo:min(m)
    hi:max(m)
  end
  info.space = 'world'
  return lo, hi, info
end

-- ---------------------------------------------------------------------------
-- Lens shift: centres the projected box horizontally, as `centreHorizontally`
-- does in framing.ts. GeometryShot takes raw view/projection matrices, so the
-- engine's own matrices are read back and the projection is skewed. Which
-- matrix entry carries the shift depends on the convention, so it is probed.

local function projectedCentre(view, proj, pts)
  local lo, hi = math.huge, -math.huge
  for _, p in ipairs(pts) do
    local v = view:transform(vec4(p.x, p.y, p.z, 1))
    local c = proj:transform(v)
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
    local c1 = projectedCentre(view, probe, worldPts)
    local slope = (c1 - c0) / 0.1
    if math.abs(slope) > 0.5 then
      local fixed = proj:clone()
      fixed[entry[1]][entry[2]] = fixed[entry[1]][entry[2]] - c0 / slope
      return fixed, c0, entry[1] .. '.' .. entry[2]
    end
  end
  return nil, c0, nil
end

-- ---------------------------------------------------------------------------
-- Colour pipeline: the shot is rendered in HDR (float texture, linear light)
-- and brought to screen colours here, by the same tone mapping as Pit Box's
-- renderer (three.js `NeutralToneMapping`, the Khronos PBR Neutral curve) and
-- an sRGB encode. Without this step the PNG held the raw HDR values clipped
-- at 1: highlights burnt, orange turned yellow (runs 1 to 3). YEBIS, the
-- game's own post-processing, would do it but loses the transparent
-- background, hence our own pass. The 2× supersampled shot is averaged down
-- by the bilinear sample at the half-resolution pixel centres.
--
-- Relighting (run 5): the template's lamps cannot be placed in the game
-- (`ac.LightSource` does not reach a GeometryShot), so they are added here. Two extra shots from the same camera
-- give the surface colour (`SampleColor`) and the world-space normal
-- (`SampleNormal`); each lamp adds `albedo × max(0, n·L) × intensity`, with L
-- the lamp direction placed as `placeLights` does in gridThumbs.svelte.ts.
--
-- Last, the preset's colour grade (`gradeColours` of look.ts: cool tint then
-- saturation, on the encoded values), which Pit Box applies to Catalogue.

local TONEMAP = [[
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
// Pit Box's reflection environment (preview3d/showroomEnvironment.ts), in the
// car's frame: near-black walls, a bright ceiling panel above the car, two
// faint side panels; scaled by the environment intensity of the grid rig.
float3 studio(float3 r) {
  float up = dot(r, gCarUp), side = abs(dot(r, gCarSide));
  float ceiling = smoothstep(0.65, 0.8, up) * 6;
  float fill = smoothstep(0.6, 0.8, side) * (1 - smoothstep(0.5, 0.65, abs(up - 0.2))) * 0.05;
  return (0.01 + ceiling + fill) * 0.55;
}
float4 main(PS_IN pin) {
  float4 c = txShot.SampleLevel(samLinearClamp, pin.Tex, 0);
  if (c.a <= 0) return 0;
  // Edges were blended over a transparent black clear: premultiplied.
  float3 rgb = max(c.rgb / c.a, 0);
  if (gEnv > 0) {
    // Reflectivity of each pixel: the same view under the game's white
    // environment minus under its black one. What stays under the black one
    // is everything but reflections (ambient, emissive).
    float4 cb = txBlack.SampleLevel(samLinearClamp, pin.Tex, 0);
    float4 cw = txWhite.SampleLevel(samLinearClamp, pin.Tex, 0);
    float3 black = max(cb.rgb / max(cb.a, 0.0001), 0);
    float3 refl = max(cw.rgb / max(cw.a, 0.0001) - black, 0);
    float4 nrmE = txNormal.SampleLevel(samLinearClamp, pin.Tex, 0);
    float3 nE = normalize(nrmE.xyz / max(nrmE.a, 0.0001) * 2 - 1);
    float3 env = gEnv == 1 ? gEnvLevel : studio(reflect(-normalize(gView), nE)) * gEnvLevel;
    rgb = black + refl * env;
  }
  if (gRelight > 0) {
    float4 alb = txAlbedo.SampleLevel(samLinearClamp, pin.Tex, 0);
    float4 nrm = txNormal.SampleLevel(samLinearClamp, pin.Tex, 0);
    float3 n = nrm.xyz / max(nrm.a, 0.0001);
    n = normalize(n * 2 - 1);
    float3 albedo = max(alb.rgb / max(alb.a, 0.0001), 0);
    if (gLinear > 0) albedo = pow(albedo, 2.2);
    float diffuse = gKey.w * saturate(dot(n, gKey.xyz)) + gFill.w * saturate(dot(n, gFill.xyz))
      + gRim.w * saturate(dot(n, gRim.xyz));
    rgb += albedo * diffuse * gRelight;
    // Specular highlight of each lamp (Blinn-Phong, one glossiness for every
    // surface: the passes carry no material). Chrome and headlight reflectors
    // have a near-black surface colour and live on reflections only, so the
    // diffuse term above leaves them dark (run 6, the Supra's left headlight).
    float3 v = normalize(gView);
    float spec = gKey.w * pow(saturate(dot(n, normalize(gKey.xyz + v))), gGloss)
      + gFill.w * pow(saturate(dot(n, normalize(gFill.xyz + v))), gGloss)
      + gRim.w * pow(saturate(dot(n, normalize(gRim.xyz + v))), gGloss);
    rgb += spec * gSpecular * (nrm.a > 0.5 ? 1 : 0);
  }
  float3 outc = toSrgb(neutral(rgb * gExposure));
  if (gGrade > 0) {
    outc *= 1 + gTint;
    float luma = dot(outc, float3(0.2126, 0.7152, 0.0722));
    outc = luma + (outc - luma) * gSaturation;
  }
  return float4(saturate(outc), saturate(c.a));
}
]]

-- ---------------------------------------------------------------------------
-- Lamps: the template's three, placed as `placeLights` does in
-- gridThumbs.svelte.ts — key 45° off the camera azimuth and 35° up, fill
-- -135° and 20°, rim 175° and 12°; fill and rim as a share of the key
-- (Catalogue: 25 % and 60 %).

local LAMPS = { { 'key', 45, 35, 1 }, { 'fill', -135, 20, 0.25 }, { 'rim', 175, 12, 0.6 } }

-- Lamp positions in world space, at four radii from the car.
local function lampPositions(state)
  local list = {}
  local base = math.atan2(state.camDir.x, state.camDir.z)
  for _, lamp in ipairs(LAMPS) do
    local angle, elevation = base + rad(lamp[2]), rad(lamp[3])
    local local_ = state.center + vec3(math.cos(elevation) * math.sin(angle), math.sin(elevation),
      math.cos(elevation) * math.cos(angle)) * (state.radius * 4)
    list[#list + 1] = { pos = state.modelToWorld:transformPoint(local_), share = lamp[4] }
  end
  return list
end

-- ---------------------------------------------------------------------------
-- Passes, all from the same camera.
--
-- Kept here so they are not tried again:
-- - run 1: the default neutral lighting (ambient 3, white-to-black reflection
--   gradient) burns highlights and tints silver paint blue;
--   `setOriginalLighting` after `setBestSceneShotQuality` changes nothing;
--   YEBIS composites the blurred track scene behind the car and loses the
--   transparent background;
-- - run 2: after `setBestSceneShotQuality` the shot is lit by the track's own
--   sun and the ambient colour has no effect; SimplifiedYEBIS leaves the car
--   half transparent;
-- - run 3: ambient 1.5 to 0.6 gave nearly identical images — the raw HDR was
--   clipped (see the colour pipeline above);
-- - run 4: the reflection gradient washes every paint out to pastel; the
--   showroom cubemap (`'menu'`) gives believable colours; the track sun stays
--   overexposed even at ×0.6.

local function showroom(s)
  s:setShadersType(render.ShadersType.Main):setMaxLayer(5)
  s:setAmbientColor(rgbm(1, 1, 1, 0.5))
  s:setReflectionColor(rgbm(0.35, 0.35, 0.35, 1), rgbm(0.08, 0.08, 0.08, 1))
  s:overrideReflectionCubemap('menu')
end

local PASSES = {
  { id = 'base', setup = showroom },
  -- Run 9: the reflectivity of each pixel, from the same view under the
  -- game's white and black environments (run 8: the dark headlight is its
  -- chrome reflecting the dark side of the showroom cubemap).
  { id = 'black', setup = function(s) showroom(s) s:overrideReflectionCubemap('black') end },
  { id = 'white', setup = function(s) showroom(s) s:overrideReflectionCubemap('white') end },
  -- Opaque only: glass would hide the cabin from the relighting.
  { id = 'normal', opaque = true, setup = function(s) s:setShadersType(render.ShadersType.SampleNormal) end },
  { id = 'albedo', opaque = true, setup = function(s) s:setShadersType(render.ShadersType.SampleColor) end },
}

-- Run 5, kept so it is not tried again: `ac.LightSource` lamps have no effect
-- on a shot under the original lighting either — the game's lights do not
-- reach a GeometryShot at all. `SampleNormal` is the world-space normal
-- encoded as n·0.5+0.5 (measured: hood y 0.95, flank x 0.9).

-- What gets written: one image per output, each a combination of passes.
-- `relight` scales the added lamps (`albedo × cos × relight` for the key);
-- `specular` scales their highlights. Run 6 settled the surface colour: used
-- as the game hands it over (decoding it from sRGB first, as three.js does
-- with its textures, gave an orange too red and too dark), hence `linear`
-- stays off.
local GRADE = { tint = vec3(-0.03, 0.02, 0.055), saturation = 0.75 }
local OUTPUTS = {
  { name = 'menu', exposure = 0.8, grade = true, relight = 2.0, specular = 0.3 },
  { name = 'flat05', exposure = 0.8, grade = true, relight = 2.0, specular = 0.3, env = 1, envLevel = 0.5 },
  { name = 'studio05', exposure = 0.8, grade = true, relight = 2.0, specular = 0.3, env = 2, envLevel = 0.5 },
  { name = 'studio10', exposure = 0.8, grade = true, relight = 2.0, specular = 0.3, env = 2, envLevel = 1.0 },
  { name = 'studio20', exposure = 0.8, grade = true, relight = 2.0, specular = 0.3, env = 2, envLevel = 2.0 },
}

local function carIndexOf(id)
  for i = 0, ac.getSim().carsCount - 1 do
    if ac.getCarID(i) == id then return i end
  end
  return nil
end

local function renderPass(state, pass)
  local shot = ac.GeometryShot(state.root, vec2(job.width * job.supersample, job.height * job.supersample), 1, false,
    render.AntialiasingMode.None, render.TextureFormat.R16G16B16A16.Float)
  shot:setClearColor(rgbm(0, 0, 0, 0)):setOpaqueAlphaFix(true):setTransparentPass(not pass.opaque)
  shot:setClippingPlanes(state.near, state.far)
  pass.setup(shot)
  if state.view then
    -- Same matrices as the first pass, lens shift included.
    shot:update(state.view, state.proj, state.pos)
  else
    shot:update(state.pos, state.look, state.up, job.template.fov)
    local proj, centre, entry = shiftedProjection(shot:viewMatrix(), shot:projectionMatrix(), state.worldCorners)
    state.shift = { centre = centre, entry = entry }
    state.view = shot:viewMatrix():clone()
    state.proj = proj or shot:projectionMatrix():clone()
    shot:update(state.view, state.proj, state.pos)
  end
  return shot
end

-- Builds the list of steps for one car: each step is a function run on its
-- own frame, so nothing reads a texture rendered on the same frame.
local function planCar(entry, steps)
  local record = { id = entry.id, files = {}, status = 'pending' }
  done.cars[#done.cars + 1] = record
  local state = {}
  local shots = {}

  steps[#steps + 1] = function()
    local index = carIndexOf(entry.id)
    if not index then
      record.status = 'not_in_session'
      return
    end
    local car = ac.getCar(index)
    -- A car standing in its pit box gets its driver door opened by the game
    -- (seen on the Supra of the second run).
    record.doorWasOpen = car.isDriverDoorOpen
    ac.setDriverDoorOpen(index, false, true)
    record.skin = ac.getCarSkinID(index)
    local root = ac.findNodes('carRoot:' .. index)
    local modelToWorld = root:getWorldTransformationRaw():clone()
    local worldToModel = modelToWorld:inverse()
    local bmin, bmax, box = measureBox(root, worldToModel, car)
    record.box = box
    record.activeLOD = car.activeLOD

    local camLocal, targetLocal, distance = fitOrbit(bmin, bmax, job.template, job.width / job.height)
    local pos = modelToWorld:transformPoint(camLocal)
    local target = modelToWorld:transformPoint(targetLocal)
    local upWorld = modelToWorld:transformPoint(targetLocal + vec3(0, 1, 0)) - target
    state.pos, state.look, state.up = pos, (target - pos):normalize(), upWorld:normalize()
    state.near = math.max(distance / 100, 0.01)
    state.far = distance * 4
    state.root = root
    state.modelToWorld = modelToWorld
    state.center = (bmin + bmax) / 2
    state.centerWorld = modelToWorld:transformPoint(state.center)
    state.radius = (bmax - bmin):length() / 2
    state.camDir = camLocal - targetLocal
    local origin = modelToWorld:transformPoint(vec3(0, 0, 0))
    state.carUp = (modelToWorld:transformPoint(vec3(0, 1, 0)) - origin):normalize()
    state.carSide = (modelToWorld:transformPoint(vec3(1, 0, 0)) - origin):normalize()
    state.worldCorners = {}
    for _, c in ipairs(corners(bmin, bmax)) do
      state.worldCorners[#state.worldCorners + 1] = modelToWorld:transformPoint(c)
    end
    state.lamps = {}
    for i, lamp in ipairs(lampPositions(state)) do
      state.lamps[i] = (lamp.pos - state.centerWorld):normalize()
    end
    record.distance = distance
  end

  for _, pass in ipairs(PASSES) do
    steps[#steps + 1] = function()
      if not state.root then return end
      local ok, err = pcall(function() shots[pass.id] = renderPass(state, pass) end)
      if not ok then record.files[pass.id] = 'error: ' .. tostring(err) end
      record.shift = state.shift
    end
  end

  local canvases = {}
  steps[#steps + 1] = function()
    if not shots.base then return end
    local function lamp(i)
      local d, share = state.lamps[i], LAMPS[i][4]
      return vec4(d.x, d.y, d.z, share)
    end
    for _, out in ipairs(OUTPUTS) do
      if shots[out.base or 'base'] then
        local canvas = ui.ExtraCanvas(vec2(job.width, job.height), 1, render.AntialiasingMode.None)
        local ok, err = pcall(function()
          canvas:updateWithShader({
            textures = { txShot = shots[out.base or 'base'], txAlbedo = shots.albedo or false, txNormal = shots.normal or false,
              txBlack = shots.black or false, txWhite = shots.white or false },
            values = {
              gExposure = out.exposure, gRelight = out.relight or 0, gLinear = out.linear and 1 or 0,
              gEnv = out.env or 0, gEnvLevel = out.envLevel or 0, gCarUp = state.carUp, gCarSide = state.carSide,
              gSpecular = out.specular or 0, gGloss = 60, gView = state.look * -1,
              gGrade = out.grade and 1 or 0, gTint = GRADE.tint, gSaturation = GRADE.saturation,
              gKey = lamp(1), gFill = lamp(2), gRim = lamp(3),
            },
            shader = TONEMAP,
          })
        end)
        if ok then
          canvases[#canvases + 1] = { name = out.name, canvas = canvas }
        else
          record.files[out.name] = 'shader error: ' .. tostring(err)
        end
      end
    end
  end

  steps[#steps + 1] = function()
    for _, c in ipairs(canvases) do
      local file = JOB_DIR .. '/out/' .. entry.id .. '__' .. c.name .. '.png'
      local ok, err = pcall(function() c.canvas:save(file, ac.ImageFormat.PNG) end)
      record.files[c.name] = ok and file or ('error: ' .. tostring(err))
      c.canvas:dispose()
    end
    for _, shot in pairs(shots) do shot:dispose() end
    record.status = 'done'
    saveDone()
  end
end

-- Which folders accept a write from an app script (doc: "check which ones").
local function probeWrites()
  local targets = { documents = JOB_DIR .. '/probe.txt' }
  for key, path in pairs(job.probes or {}) do targets[key] = path end
  for key, path in pairs(targets) do
    local ok, result = pcall(io.save, path, 'pitbox_shot probe')
    done.probes[key] = { path = path, ok = ok and result == true }
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
    if waited < (job.warmup or 6) then return end
    io.createDir(JOB_DIR .. '/out')
    probeWrites()
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
      if not ok then ac.error('pitbox_shot: ' .. tostring(err)) ; done.lastError = tostring(err) end
      stepIndex = stepIndex + 1
      return
    end
    done.finished = os.date('%Y-%m-%dT%H:%M:%S')
    status = 'finished'
    saveDone()
    if job.quit then quitAt = os.preciseClock() + 1 end
  end
end

function windowMain()
  ui.text('Pit Box Shot: ' .. status)
  if steps then ui.text(string.format('%d / %d', math.min(stepIndex - 1, #steps), #steps)) end
end
