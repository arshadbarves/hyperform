/** The 3D pet. This folder is the only place in the app that imports `three`. */

export { disposeRenderer, environment, renderView, webgl, webglSupported } from "./renderer.svelte";
export { petFrames, type FrameClient, type FrameHandle, type FrameRate } from "./scheduler";
export {
  bodyColor,
  mixColors,
  petAlpha,
  petColor,
  propAlpha,
  propColor,
  refreshPalette,
  toneColor,
  type PetColorName,
  type PropColorName,
} from "./palette";
export { FOV, FULL_SPAN, PetScene, faster, type PetSceneState } from "./petScene";
