import { PMREMGenerator, SRGBColorSpace, WebGLRenderer, type Camera, type Scene, type Texture, type WebGLRenderTarget } from "three";
import { RoomEnvironment } from "three/examples/jsm/environments/RoomEnvironment.js";

/** Sharper than this costs fill rate and looks no different at the pet's size. */
const MAX_PIXEL_RATIO = 2;
/** How much the room is blurred before it becomes the reflection map. */
const ENV_BLUR = 0.04;

let renderer: WebGLRenderer | null = null;
let failed = false;
let env: WebGLRenderTarget | null = null;
/** The GL canvas in device pixels: the largest view asked for so far. */
let buffer = { width: 0, height: 0 };
const status = $state({ lost: false, restores: 0 });

/**
 * The shared WebGL context's health, reactive. While `lost` is true the pet
 * draws flat; `restores` counts each time the context came back, so a view
 * can take the new `environment()` and redraw.
 */
export const webgl: { readonly lost: boolean; readonly restores: number } = {
  get lost() {
    return status.lost;
  },
  get restores() {
    return status.restores;
  },
};

function onLost(event: Event) {
  event.preventDefault();
  status.lost = true;
}

/** The reflection map lived in the old context, so it is built again on the next ask. */
function onRestored() {
  env = null;
  status.lost = false;
  status.restores += 1;
}

function create(): WebGLRenderer | null {
  if (typeof document === "undefined") return null;
  try {
    const canvas = document.createElement("canvas");
    // The pet never needs the discrete GPU, and asking for it would switch dual-GPU Macs onto it.
    const gl = new WebGLRenderer({ canvas, alpha: true, antialias: true, powerPreference: "low-power" });
    gl.setPixelRatio(1);
    gl.outputColorSpace = SRGBColorSpace;
    gl.setClearColor(0x000000, 0);
    canvas.addEventListener("webglcontextlost", onLost);
    canvas.addEventListener("webglcontextrestored", onRestored);
    return gl;
  } catch {
    return null;
  }
}

/** The one renderer every pet view shares, made on first use; null without WebGL. */
function shared(): WebGLRenderer | null {
  if (!renderer && !failed) {
    renderer = create();
    failed = !renderer;
  }
  return renderer;
}

/** Whether this webview can draw the 3D pet (a WebGL 2 context could be made). */
export function webglSupported(): boolean {
  return shared() !== null;
}

/**
 * The reflection map every pet material shares: three's `RoomEnvironment`,
 * prefiltered once. Null without a context; after a restore it is a new
 * texture, so set it on the scene again when `webgl.restores` changes.
 */
export function environment(): Texture | null {
  const gl = shared();
  if (!gl || status.lost) return null;
  if (!env) {
    const pmrem = new PMREMGenerator(gl);
    const room = new RoomEnvironment();
    env = pmrem.fromScene(room, ENV_BLUR);
    room.dispose();
    pmrem.dispose();
  }
  return env.texture;
}

/** Grows the GL canvas to hold a view this size; it never shrinks, so views of every size share it without reallocating. */
function fit(gl: WebGLRenderer, width: number, height: number) {
  if (width <= buffer.width && height <= buffer.height) return;
  buffer = { width: Math.max(width, buffer.width), height: Math.max(height, buffer.height) };
  gl.setSize(buffer.width, buffer.height, false);
}

/**
 * Draws `scene` through `camera` at `width` × `height` CSS pixels into the
 * view's own 2D `target` canvas: it renders into the bottom-left corner of
 * the shared GL canvas, then copies that corner across. The target's
 * backing store is sized to device pixels; its CSS size, and the camera's
 * aspect, are the caller's. Returns whether it drew (not while the context
 * is lost, without WebGL, or at zero size).
 */
export function renderView(target: HTMLCanvasElement, scene: Scene, camera: Camera, width: number, height: number): boolean {
  const gl = shared();
  if (!gl || status.lost || width <= 0 || height <= 0) return false;
  const ctx = target.getContext("2d");
  if (!ctx) return false;
  const ratio = Math.min(MAX_PIXEL_RATIO, globalThis.devicePixelRatio || 1);
  const w = Math.max(1, Math.round(width * ratio));
  const h = Math.max(1, Math.round(height * ratio));
  fit(gl, w, h);
  gl.setViewport(0, 0, w, h);
  gl.setScissor(0, 0, w, h);
  gl.setScissorTest(true);
  gl.render(scene, camera);
  if (target.width !== w || target.height !== h) {
    target.width = w;
    target.height = h;
  } else {
    ctx.clearRect(0, 0, w, h);
  }
  ctx.drawImage(gl.domElement, 0, buffer.height - h, w, h, 0, 0, w, h);
  return true;
}

/** Frees the renderer, its reflection map and its context; the next use makes a fresh one. */
export function disposeRenderer() {
  env?.dispose();
  env = null;
  if (renderer) {
    renderer.domElement.removeEventListener("webglcontextlost", onLost);
    renderer.domElement.removeEventListener("webglcontextrestored", onRestored);
    renderer.dispose();
    renderer.forceContextLoss();
  }
  renderer = null;
  failed = false;
  buffer = { width: 0, height: 0 };
  status.lost = false;
}

import.meta.hot?.dispose(disposeRenderer);
