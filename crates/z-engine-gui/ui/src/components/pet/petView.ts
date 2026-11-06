import type { Component } from "svelte";
import type { PetAccessory, PetLook, PetStage } from "$lib/domain/pet/looks";
import type { Point } from "$lib/domain/pet/physics";
import type { PetPose } from "$lib/domain/pet/pose";
import type { PetFraming } from "$lib/domain/pet/portrait";
import type { PetRoutine } from "$lib/domain/pet/routines";

/** What a pet view takes: the flat pet's props, and what only the 3D pet uses. */
export interface PetViewProps {
  pose: PetPose;
  look?: PetLook;
  stage?: PetStage;
  wearing?: PetAccessory | null;
  /** Plan progress drawn as a ring around the pet, 0..1. */
  progress?: number | null;
  /** Running agents and jobs, drawn orbiting it (at most four). */
  helpers?: number;
  size?: number;
  /** Which way it faces, so its eyes follow `lookAt` the right way round. */
  facing?: 1 | -1;
  /** An idle routine or trick it is playing. */
  routine?: PetRoutine | null;
  /** A point on screen to look at (the caret, the pointer). */
  lookAt?: Point | null;
  /** The motion engine's facing, -1 (left) .. 1 (right), as a turn of the body; 0 faces you. */
  turn?: number;
  /** Feet lifted [left, right] and the body's bob as it walks, in its 32-unit space. */
  lift?: readonly [number, number];
  bob?: number;
  /** All of the pet, or a close-up of its face (the island's portrait). */
  framing?: PetFraming;
  /** The portrait's head turn and nod, radians (`portraitLook`). */
  head?: { yaw: number; pitch: number } | null;
}

export interface Pet3DModule {
  View: Component<PetViewProps>;
  webgl: { readonly lost: boolean };
}

let loading: Promise<Pet3DModule | null> | null = null;

/**
 * The 3D pet, loaded on first ask: `three` comes in its own chunk, so the
 * app starts without it. Null where the webview has no WebGL or the chunk
 * fails to load; the flat pet stays then.
 */
export function loadPet3D(): Promise<Pet3DModule | null> {
  loading ??= Promise.all([import("./Pet3D.svelte"), import("$lib/pet3d")])
    .then(([view, gl]) => (gl.webglSupported() ? { View: view.default, webgl: gl.webgl } : null))
    .catch(() => null);
  return loading;
}
