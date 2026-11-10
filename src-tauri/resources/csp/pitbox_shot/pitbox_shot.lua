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
-- Variants: one lighting setup each, rendered from the same camera.
--
-- First run, kept here so it is not tried again: the default neutral lighting
-- (ambient 3, white-to-black reflection gradient) burns highlights and tints
-- silver paint blue; `setOriginalLighting` after `setBestSceneShotQuality`
-- changes nothing; YEBIS composites the blurred track scene behind the car and
-- loses the transparent background.
--
-- Second run: after `setBestSceneShotQuality` the ambient colour has no effect
-- (ambient 1.5 and 0.9 gave identical images, both saturated, orange turning
-- yellow): the shot is lit by the track's own sun. SimplifiedYEBIS tonemaps
-- but leaves the car half transparent. Hence this third set, on Main shaders
-- with the neutral lighting kept, at decreasing ambient levels.

local function main(s, ambient, zenith, horizon)
  s:setShadersType(render.ShadersType.Main):setMaxLayer(5)
  s:setAmbientColor(rgbm(1, 1, 1, ambient))
  s:setReflectionColor(rgbm(zenith, zenith, zenith, 1), rgbm(horizon, horizon, horizon, 1))
end

local VARIANTS = {
  { id = 'main_a15', aa = render.AntialiasingMode.ExtraSharpCMAA,
    setup = function(s) main(s, 1.5, 0.5, 0.1) end },
  { id = 'main_a10', aa = render.AntialiasingMode.ExtraSharpCMAA,
    setup = function(s) main(s, 1.0, 0.4, 0.08) end },
  { id = 'main_a06', aa = render.AntialiasingMode.ExtraSharpCMAA,
    setup = function(s) main(s, 0.6, 0.3, 0.05) end },
  { id = 'best_neutral', aa = render.AntialiasingMode.ExtraSharpCMAA,
    setup = function(s)
      s:setBestSceneShotQuality():setSky(false):setParticles(false):setGrass(false):setOriginalLighting(false)
      s:setAmbientColor(rgbm(1, 1, 1, 1.0)):setReflectionColor(rgbm(0.4, 0.4, 0.4, 1), rgbm(0.08, 0.08, 0.08, 1))
    end },
}

local function carIndexOf(id)
  for i = 0, ac.getSim().carsCount - 1 do
    if ac.getCarID(i) == id then return i end
  end
  return nil
end

-- Builds the list of steps for one car: each step is a function run on its
-- own frame, so a save never reads a texture rendered on the same frame.
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
    if job.mirrorX then
      camLocal.x, targetLocal.x = -camLocal.x, -targetLocal.x
    end
    local pos = modelToWorld:transformPoint(camLocal)
    local target = modelToWorld:transformPoint(targetLocal)
    local upWorld = modelToWorld:transformPoint(targetLocal + vec3(0, 1, 0)) - target
    state.pos, state.look, state.up = pos, (target - pos):normalize(), upWorld:normalize()
    state.near = math.max(distance / 100, 0.01)
    state.far = distance * 4
    state.root = root
    state.worldCorners = {}
    for _, c in ipairs(corners(bmin, bmax)) do
      state.worldCorners[#state.worldCorners + 1] = modelToWorld:transformPoint(c)
    end
    record.distance = distance
  end

  for _, variant in ipairs(VARIANTS) do
    -- Unshifted shots are produced on demand only: the lens shift was proven
    -- on the first run (projected centre 0.065 brought back to the middle).
    for _, shift in ipairs(job.unshifted and { false, true } or { true }) do
      local name = variant.id .. (shift and '_shift' or '')
      local shot
      steps[#steps + 1] = function()
        if not state.root then return end
        local ok, err = pcall(function()
          shot = ac.GeometryShot(state.root, vec2(job.width * job.supersample, job.height * job.supersample), 1, false,
            variant.aa)
          shot:setClearColor(rgbm(0, 0, 0, 0)):setOpaqueAlphaFix(true):setTransparentPass(true)
          shot:setClippingPlanes(state.near, state.far)
          variant.setup(shot)
          shot:update(state.pos, state.look, state.up, job.template.fov)
          if shift then
            local proj, centre, entry = shiftedProjection(shot:viewMatrix(), shot:projectionMatrix(), state.worldCorners)
            record.shift = { centre = centre, entry = entry }
            if proj then shot:update(shot:viewMatrix(), proj, state.pos) end
          end
        end)
        if not ok then
          record.files[name] = 'error: ' .. tostring(err)
          shot = nil
        end
      end
      steps[#steps + 1] = function()
        if not shot then return end
        local file = JOB_DIR .. '/out/' .. entry.id .. '__' .. name .. '.png'
        local ok, err = pcall(function() shot:save(file, ac.ImageFormat.PNG) end)
        record.files[name] = ok and file or ('error: ' .. tostring(err))
        shot:dispose()
        shot = nil
        record.status = 'done'
        saveDone()
      end
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
