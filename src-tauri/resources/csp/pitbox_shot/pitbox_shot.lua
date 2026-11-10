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
float4 main(PS_IN pin) {
  float4 c = txShot.SampleLevel(samLinearClamp, pin.Tex, 0);
  if (c.a <= 0) return 0;
  // Edges were blended over a transparent black clear: premultiplied.
  float3 rgb = c.rgb / c.a;
  return float4(toSrgb(neutral(max(rgb, 0) * gExposure)), saturate(c.a));
}
]]

-- ---------------------------------------------------------------------------
-- Lights: the template's three lamps, placed as `placeLights` does in
-- gridThumbs.svelte.ts — key 45° off the camera azimuth and 35° up, fill
-- -135° and 20°, rim 175° and 12° — at four radii from the car. Pit Box uses
-- directional lights; AC has only point and spot lights, so these are point
-- lights far enough, with a range long enough, to fall off very little across
-- the car. Their brightness scale against three.js is unknown: `job.lights`
-- multiplies them all, and the run compares a few values.
--
-- Run 4: **no visible effect** — with lamps at 0, 1 and 3, the images are the
-- same. `ac.LightSource` lights do not seem to reach a GeometryShot; kept so
-- the next attempt (shader type, lights linked to the car node) starts here.

local LAMPS = { { 'key', 45, 35, 1 }, { 'fill', -135, 20, 0.25 }, { 'rim', 175, 12, 0.6 } }

local function placeLamps(state, strength)
  local lights = {}
  local base = math.atan2(state.camDir.x, state.camDir.z)
  for _, lamp in ipairs(LAMPS) do
    local angle, elevation = base + rad(lamp[2]), rad(lamp[3])
    local local_ = state.center + vec3(math.cos(elevation) * math.sin(angle), math.sin(elevation),
      math.cos(elevation) * math.cos(angle)) * (state.radius * 4)
    local light = ac.LightSource(ac.LightType.Regular)
    light.position = state.modelToWorld:transformPoint(local_)
    light.direction = (state.centerWorld - light.position):normalize()
    light.spot = 0
    light.range = state.radius * 16
    light.rangeGradientOffset = 0.9
    light.diffuseConcentration = 1
    light.specularMultiplier = 1
    light.shadows = false
    light.affectsCars = true
    light.showInReflections = false
    local v = strength * lamp[4]
    light.color = rgb(v, v, v)
    lights[#lights + 1] = light
  end
  return lights
end

-- ---------------------------------------------------------------------------
-- Variants: one lighting setup each, rendered from the same camera.
--
-- Kept here so they are not tried again:
-- - run 1: the default neutral lighting (ambient 3, white-to-black reflection
--   gradient) burns highlights and tints silver paint blue;
--   `setOriginalLighting` after `setBestSceneShotQuality` changes nothing;
--   YEBIS composites the blurred track scene behind the car and loses the
--   transparent background;
-- - run 2: after `setBestSceneShotQuality` the ambient colour has no effect,
--   the shot is lit by the track's own sun; SimplifiedYEBIS tonemaps but
--   leaves the car half transparent;
-- - run 3: on Main shaders with neutral lighting, ambient 1.5 to 0.6 gave
--   nearly identical images — the strong reflection dominates, and the raw HDR
--   was clipped (see the colour pipeline above).

local function neutral(s, ambient, zenith, horizon)
  s:setShadersType(render.ShadersType.Main):setMaxLayer(5)
  s:setAmbientColor(rgbm(1, 1, 1, ambient))
  s:setReflectionColor(rgbm(zenith, zenith, zenith, 1), rgbm(horizon, horizon, horizon, 1))
end

local VARIANTS = {
  -- Neutral lighting alone, reflections dimmed: what the lamps add is measured against this.
  { id = 'ambient_only', lights = 0, setup = function(s) neutral(s, 0.5, 0.35, 0.08) end },
  { id = 'lamps', lights = 1, setup = function(s) neutral(s, 0.5, 0.35, 0.08) end },
  { id = 'lamps_strong', lights = 3, setup = function(s) neutral(s, 0.5, 0.35, 0.08) end },
  { id = 'lamps_showroom', lights = 1,
    setup = function(s) neutral(s, 0.5, 0.35, 0.08) s:overrideReflectionCubemap('menu') end },
  -- The track's own sun, for reference, through the same colour pipeline.
  { id = 'track_sun', lights = 0,
    setup = function(s) s:setBestSceneShotQuality():setSky(false):setParticles(false):setGrass(false) end },
}

local EXPOSURES = { 0.6, 1.0, 1.6 }

local function carIndexOf(id)
  for i = 0, ac.getSim().carsCount - 1 do
    if ac.getCarID(i) == id then return i end
  end
  return nil
end

-- Builds the list of steps for one car: each step is a function run on its
-- own frame, so nothing reads a texture rendered on the same frame.
local function planCar(entry, steps)
  local record = { id = entry.id, files = {}, status = 'pending' }
  done.cars[#done.cars + 1] = record
  local state = {}

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
    record.skin = ac.getCarSkinID and ac.getCarSkinID(index) or nil
    local root = ac.findNodes('carRoot:' .. index)
    local modelToWorld = root:getWorldTransformationRaw():clone()
    local worldToModel = modelToWorld:inverse()
    local bmin, bmax, box = measureBox(root, worldToModel, car)
    record.box = box
    record.boxModel = { min = { bmin.x, bmin.y, bmin.z }, max = { bmax.x, bmax.y, bmax.z } }
    record.activeLOD = car.activeLOD
    record.speedKmh = car.speedKmh

    local t = job.template
    local aspect = job.width / job.height
    local camLocal, targetLocal, distance = fitOrbit(bmin, bmax, t, aspect)
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
    state.worldCorners = {}
    for _, c in ipairs(corners(bmin, bmax)) do
      state.worldCorners[#state.worldCorners + 1] = modelToWorld:transformPoint(c)
    end
    record.distance = distance
  end

  for _, variant in ipairs(VARIANTS) do
    local shot, lights, canvases
    -- Frame 1: lamps on. Lights placed on the same frame as the shot might not
    -- be registered yet.
    steps[#steps + 1] = function()
      if not state.root then return end
      if variant.lights > 0 then lights = placeLamps(state, variant.lights * (job.lights or 4)) end
    end
    -- Frame 2: render in HDR, with the lens shift (centres the projected box).
    steps[#steps + 1] = function()
      if not state.root then return end
      local ok, err = pcall(function()
        shot = ac.GeometryShot(state.root, vec2(job.width * job.supersample, job.height * job.supersample), 1, false,
          render.AntialiasingMode.None, render.TextureFormat.R16G16B16A16.Float)
        shot:setClearColor(rgbm(0, 0, 0, 0)):setOpaqueAlphaFix(true):setTransparentPass(true)
        shot:setClippingPlanes(state.near, state.far)
        variant.setup(shot)
        shot:update(state.pos, state.look, state.up, job.template.fov)
        local proj, centre, entry = shiftedProjection(shot:viewMatrix(), shot:projectionMatrix(), state.worldCorners)
        record.shift = { centre = centre, entry = entry }
        if proj then shot:update(shot:viewMatrix(), proj, state.pos) end
      end)
      if not ok then
        record.files[variant.id] = 'error: ' .. tostring(err)
        shot = nil
      end
    end
    -- Frame 3: tone map at each exposure.
    steps[#steps + 1] = function()
      if lights then for _, l in ipairs(lights) do l:dispose() end lights = nil end
      if not shot then return end
      canvases = {}
      for _, exposure in ipairs(EXPOSURES) do
        local canvas = ui.ExtraCanvas(vec2(job.width, job.height), 1, render.AntialiasingMode.None)
        local ok, err = pcall(function()
          canvas:updateWithShader({ textures = { txShot = shot }, values = { gExposure = exposure }, shader = TONEMAP })
        end)
        if not ok then record.files[variant.id] = 'tonemap error: ' .. tostring(err) end
        canvases[#canvases + 1] = { exposure = exposure, canvas = canvas }
      end
    end
    -- Frame 4: save.
    steps[#steps + 1] = function()
      if not canvases then return end
      for _, c in ipairs(canvases) do
        local name = string.format('%s_e%02d', variant.id, math.floor(c.exposure * 10 + 0.5))
        local file = JOB_DIR .. '/out/' .. entry.id .. '__' .. name .. '.png'
        local ok, err = pcall(function() c.canvas:save(file, ac.ImageFormat.PNG) end)
        record.files[name] = ok and file or ('error: ' .. tostring(err))
        c.canvas:dispose()
      end
      canvases = nil
      shot:dispose()
      shot = nil
      record.status = 'done'
      saveDone()
    end
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
