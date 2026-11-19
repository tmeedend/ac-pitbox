// The interior view of the car preview (docs/SPEC-preview-3d-kn5.md
// PREVIEW§15.1): the camera flies into the driver's seat, and the mouse turns
// and moves the head instead of orbiting the car.
//
// The seat is the car's own `[GRAPHICS] DRIVEREYES`, the point the game puts
// its cockpit camera on — in the car's space, which is the converted model's
// space (the converter applies no change of frame). OrbitControls is disabled
// for the whole stay, so its own state is untouched and hands back exactly
// where it was when the camera comes out.
import type * as ThreeModule from "three";
import { reportListenerHead } from "$lib/detail/enginePlayer.svelte";
import { setDriverHeadHidden } from "./rig";
import type { ThreeScene } from "./scene";
import {
  COCKPIT_FLIGHT_ARC,
  COCKPIT_FLIGHT_MS,
  COCKPIT_FOV,
  COCKPIT_FOV_MAX,
  COCKPIT_FOV_MIN,
  COCKPIT_LEAN_SIDE,
  COCKPIT_LEAN_UP,
  COCKPIT_LOOK_SPEED,
  COCKPIT_MOVE_SPEED,
  COCKPIT_PITCH_DOWN,
  COCKPIT_PITCH_UP,
  COCKPIT_YAW_LIMIT,
} from "./tuning";

/** Where the head is turned and leaned, relative to the seat. Carried from one
 * scene to the next on a change of skin, so the view does not jump. */
export interface CockpitLook {
  /** Degrees from the nose towards +X. */
  yaw: number;
  /** Degrees above the horizon. */
  pitch: number;
  /** Metres from the eyes, sideways (x) and up (y), in the car's space. */
  leanX: number;
  leanY: number;
  fov: number;
}

/** A camera pose, as the flight interpolates it. */
interface Pose {
  position: ThreeModule.Vector3;
  quaternion: ThreeModule.Quaternion;
  fov: number;
}

export interface Cockpit {
  /** The driver's eyes, in the model's own space. */
  eyes: ThreeModule.Vector3;
  look: CockpitLook;
  /** `entering` and `leaving` fly; `inside` follows the mouse. */
  phase: "entering" | "inside" | "leaving";
  from: Pose;
  /** Start of the flight, `performance.now()` time. */
  flightAt: number;
  /** The orbit to go back to: where the camera was before it went in, or the
   * framing of the settings when the sheet opened already inside. */
  back: { position: ThreeModule.Vector3; target: ThreeModule.Vector3; fov: number };
  /** Removes the pointer listeners. */
  detach: () => void;
}

const DEFAULT_LOOK: CockpitLook = { yaw: 0, pitch: -4, leanX: 0, leanY: 0, fov: COCKPIT_FOV };

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

/** Smooth start and end: a flight that starts at full speed is a jump. */
function ease(t: number): number {
  return t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;
}

/** Is the camera in the seat, or on its way there? */
export function inCockpit(current: ThreeScene): boolean {
  return current.cockpit !== null && current.cockpit.phase !== "leaving";
}

/** The look of a scene in the seat, to carry over to its replacement. */
export function cockpitLook(current: ThreeScene): CockpitLook | null {
  return inCockpit(current) ? { ...current.cockpit!.look } : null;
}

/** The pose of the camera in the seat, for the current look. */
function seatPose(current: ThreeScene, cockpit: Cockpit): Pose {
  const { THREE, model } = current;
  model.updateWorldMatrix(true, false);
  const { yaw, pitch, leanX, leanY, fov } = cockpit.look;
  const eye = cockpit.eyes.clone().add(new THREE.Vector3(leanX, leanY, 0));
  const position = model.localToWorld(eye.clone());
  const y = (yaw * Math.PI) / 180;
  const p = (pitch * Math.PI) / 180;
  const ahead = eye.add(new THREE.Vector3(Math.sin(y) * Math.cos(p), Math.sin(p), Math.cos(y) * Math.cos(p)));
  const target = model.localToWorld(ahead);
  return { position, quaternion: lookingAt(THREE, position, target), fov };
}

/** The orientation of a camera at `position` looking at `target`. A
 * throwaway camera does the look-at: the real one's would move it mid-flight,
 * when only the destination is wanted. */
function lookingAt(
  THREE: typeof ThreeModule,
  position: ThreeModule.Vector3,
  target: ThreeModule.Vector3,
): ThreeModule.Quaternion {
  const probe = new THREE.PerspectiveCamera();
  probe.position.copy(position);
  probe.lookAt(target);
  return probe.quaternion.clone();
}

/** The pose the orbit gives for a position and a target. */
function orbitPose(current: ThreeScene, back: Cockpit["back"]): Pose {
  return {
    position: back.position.clone(),
    quaternion: lookingAt(current.THREE, back.position, back.target),
    fov: back.fov,
  };
}

function currentPose(current: ThreeScene): Pose {
  return {
    position: current.camera.position.clone(),
    quaternion: current.camera.quaternion.clone(),
    fov: current.camera.fov,
  };
}

function applyPose(current: ThreeScene, pose: Pose) {
  current.camera.position.copy(pose.position);
  current.camera.quaternion.copy(pose.quaternion);
  if (current.camera.fov !== pose.fov) {
    current.camera.fov = pose.fov;
    current.camera.updateProjectionMatrix();
  }
}

/** Interpolates a flight: along a raised arc, so that the camera goes over the
 * bodywork rather than through it. */
function flight(current: ThreeScene, from: Pose, to: Pose, t: number): Pose {
  const k = ease(t);
  const position = from.position.clone().lerp(to.position, k);
  position.y += Math.sin(Math.PI * k) * current.radius * COCKPIT_FLIGHT_ARC;
  return {
    position,
    quaternion: from.quaternion.clone().slerp(to.quaternion, k),
    fov: from.fov + (to.fov - from.fov) * k,
  };
}

/**
 * Takes the mouse over while in the seat: left button turns the head, right
 * button leans it, the wheel narrows or widens the view, a double click looks
 * ahead again. OrbitControls is disabled meanwhile and sees none of it.
 */
function attachPointer(current: ThreeScene, cockpit: Cockpit, redraw: () => void): () => void {
  const element = current.renderer.domElement;
  let dragging: "look" | "lean" | null = null;

  const down = (e: PointerEvent) => {
    if (cockpit.phase !== "inside") return;
    dragging = e.button === 2 ? "lean" : e.button === 0 ? "look" : null;
    if (dragging) element.setPointerCapture(e.pointerId);
  };
  const move = (e: PointerEvent) => {
    if (!dragging || cockpit.phase !== "inside") return;
    const look = cockpit.look;
    if (dragging === "look") {
      // Dragging right turns the head right, which is towards -X: the car
      // faces +Z, so its right-hand side is -X in a right-handed frame.
      look.yaw = clamp(look.yaw - e.movementX * COCKPIT_LOOK_SPEED, -COCKPIT_YAW_LIMIT, COCKPIT_YAW_LIMIT);
      look.pitch = clamp(look.pitch - e.movementY * COCKPIT_LOOK_SPEED, -COCKPIT_PITCH_DOWN, COCKPIT_PITCH_UP);
    } else {
      look.leanX = clamp(look.leanX - e.movementX * COCKPIT_MOVE_SPEED, -COCKPIT_LEAN_SIDE, COCKPIT_LEAN_SIDE);
      look.leanY = clamp(look.leanY - e.movementY * COCKPIT_MOVE_SPEED, -COCKPIT_LEAN_UP, COCKPIT_LEAN_UP);
    }
    redraw();
  };
  const up = (e: PointerEvent) => {
    dragging = null;
    if (element.hasPointerCapture(e.pointerId)) element.releasePointerCapture(e.pointerId);
  };
  const wheel = (e: WheelEvent) => {
    if (cockpit.phase !== "inside") return;
    e.preventDefault();
    cockpit.look.fov = clamp(cockpit.look.fov + Math.sign(e.deltaY) * 3, COCKPIT_FOV_MIN, COCKPIT_FOV_MAX);
    redraw();
  };
  const menu = (e: Event) => e.preventDefault();
  const reset = () => {
    if (cockpit.phase !== "inside") return;
    recentreCockpit(current);
  };

  element.addEventListener("pointerdown", down);
  element.addEventListener("pointermove", move);
  element.addEventListener("pointerup", up);
  element.addEventListener("pointercancel", up);
  element.addEventListener("wheel", wheel, { passive: false });
  element.addEventListener("contextmenu", menu);
  element.addEventListener("dblclick", reset);
  return () => {
    element.removeEventListener("pointerdown", down);
    element.removeEventListener("pointermove", move);
    element.removeEventListener("pointerup", up);
    element.removeEventListener("pointercancel", up);
    element.removeEventListener("wheel", wheel);
    element.removeEventListener("contextmenu", menu);
    element.removeEventListener("dblclick", reset);
  };
}

/**
 * Goes into the seat. `animate` false lands there at once — a sheet that
 * opens already in the interior view, or a skin swapped while inside. A
 * camera coming out is turned round where it is.
 */
export function enterCockpit(
  current: ThreeScene,
  eyes: [number, number, number],
  options: { animate: boolean; look?: CockpitLook | null; redraw: () => void },
) {
  const { THREE } = current;
  const leaving = current.cockpit;
  const back = leaving
    ? leaving.back
    : {
        position: current.camera.position.clone(),
        target: current.controls.target.clone(),
        fov: current.camera.fov,
      };
  leaving?.detach();
  current.controls.enabled = false;
  // The stand stops: the seat turning under the camera would be a fairground.
  current.stage.spinning = false;
  const cockpit: Cockpit = {
    eyes: new THREE.Vector3(...eyes),
    look: { ...(options.look ?? DEFAULT_LOOK) },
    phase: options.animate ? "entering" : "inside",
    from: currentPose(current),
    flightAt: performance.now(),
    back,
    detach: () => {},
  };
  cockpit.detach = attachPointer(current, cockpit, options.redraw);
  current.cockpit = cockpit;
  if (!options.animate) {
    setDriverHeadHidden(current, true);
    applyPose(current, seatPose(current, cockpit));
  }
}

/** Comes out of the seat, back to the orbit it left. */
export function leaveCockpit(current: ThreeScene, options: { animate: boolean }) {
  const cockpit = current.cockpit;
  if (!cockpit || cockpit.phase === "leaving") return;
  cockpit.from = currentPose(current);
  cockpit.flightAt = performance.now();
  cockpit.phase = "leaving";
  if (!options.animate) finishLeaving(current, cockpit);
}

function finishLeaving(current: ThreeScene, cockpit: Cockpit) {
  cockpit.detach();
  current.cockpit = null;
  setDriverHeadHidden(current, false);
  applyPose(current, orbitPose(current, cockpit.back));
  current.controls.target.copy(cockpit.back.target);
  current.controls.enabled = true;
  current.controls.update();
}

/** Looks straight ahead again, from the seat itself. */
export function recentreCockpit(current: ThreeScene) {
  if (!current.cockpit) return;
  current.cockpit.look = { ...DEFAULT_LOOK };
}

/** Frees the listeners of a scene being disposed. */
export function disposeCockpit(current: ThreeScene) {
  current.cockpit?.detach();
  current.cockpit = null;
}

/**
 * One frame of the interior view: the flight, or the seat following the
 * mouse. Returns `true` while a flight is still under way, which keeps the
 * render loop going.
 */
export function stepCockpit(current: ThreeScene, now: number): boolean {
  const cockpit = current.cockpit;
  if (!cockpit) return false;
  const t = Math.min((now - cockpit.flightAt) / COCKPIT_FLIGHT_MS, 1);
  if (cockpit.phase === "inside") {
    applyPose(current, seatPose(current, cockpit));
    return false;
  }
  if (cockpit.phase === "entering") {
    applyPose(current, flight(current, cockpit.from, seatPose(current, cockpit), t));
    // The head goes once the camera is nearly in it, not as it sets off: seen
    // from outside, a driver losing his head mid-flight is a strange sight.
    if (t > 0.75) setDriverHeadHidden(current, true);
    if (t >= 1) cockpit.phase = "inside";
    return t < 1;
  }
  applyPose(current, flight(current, cockpit.from, orbitPose(current, cockpit.back), t));
  if (t > 0.25) setDriverHeadHidden(current, false);
  if (t >= 1) finishLeaving(current, cockpit);
  return t < 1;
}

/** Tells the engine sound where the driver's head is (FMOD§6octies): the
 * eyes, leaned, relative to the car's centre, and where the head is turned. */
export function reportCockpitEar(current: ThreeScene) {
  const cockpit = current.cockpit;
  if (!cockpit || cockpit.phase === "leaving") return;
  const { yaw, pitch, leanX, leanY } = cockpit.look;
  reportListenerHead(
    [cockpit.eyes.x + leanX - current.center.x, cockpit.eyes.y + leanY - current.center.y, cockpit.eyes.z - current.center.z],
    yaw,
    pitch,
  );
}
