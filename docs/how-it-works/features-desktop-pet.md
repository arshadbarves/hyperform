# The desktop pet

The pet that lives in the title bar's island: where it goes, how it grows,
how it is drawn in 3D, and the small portrait the island keeps of it while
it roams. The island itself is on
[The desktop app's screens](features-desktop-screens.md#the-island-and-the-pet).
Terms are in the [glossary](glossary.md); the UI rules in the
[GUI UI guide](../design/gui-ui-guide.md) and which component owns what in
[GUI surfaces](../design/gui-surfaces.md#4-the-pet).

## Where it goes and how it grows

**In plain words.** The pet keeps you company like a cat that follows you
from room to room: it sits where you work while nothing is going on, runs
back to the title bar when the agent gets busy or needs you, and slowly
grows as work gets done. It only echoes the status line, never adds to it.

**How it works**
1. Where it goes follows rules, most important first: being dragged;
   something needs you (it hops into the island); the agent works (it
   rides in the island); you type (it watches from the composer's edge); a
   turn was just verified (it celebrates where it is); a menu, the palette
   or the island card is open (it tucks itself away); 3 minutes without
   activity (it naps); otherwise it rests or now and then wanders.
2. The places it can be are *perches*: slots it sits in (the island, the
   spot above the home page's question, the empty Inbox, the side panel's
   tab band) and edges it stands on (the top of the composer, the sidebar
   footer). It rests on the home spot, in the empty Inbox, else on the
   composer, and wanders between the composer, the sidebar and the panel;
   while the panel covers the stage, the stage's perches are out of reach.
   Dropped after a drag, or thrown, it lands on the nearest perch (dizzy
   after a hard throw or a shake).
3. It moves on springs: it hops, walks, turns and swings from your grip,
   and a frame loop runs only while it moves. Perches are measured on
   resize and scroll and on the status clock's half-second tick (only
   while a turn runs or the pet is lively), so an idle pet costs almost
   nothing; how drawing it costs nothing while it rests is
   [below](#how-it-is-drawn).
4. **Calm** keeps it in the island, `ui.pet.roam = false` leaves only the
   home spot, and **Off** removes it (the island shows a dot). Under Reduce
   Motion it appears at its new spot instead of walking, with no idle
   strolls or tricks.
5. It grows from live events of any chat: `turnFinished` (10 XP for a
   completed turn, 15 more if verified) and `agentUpdated` with a worktree
   applied (20 XP), plus 5 for the first of the day, which also extends the
   day streak. Each turn or apply counts once (the last 200 ids are kept),
   and reopening a chat awards nothing. Level *L* needs 25·*L*·(*L*−1) XP
   (up to 99); stages change at levels 3, 6 and 10, accessories unlock at
   2, 4, 6 and 9, idle tricks at 1, 3, 5 and 7. A new level waits in
   `pet.levelUp` until the island is not asking for you, then plays the
   `levelUp` reaction (confetti, 1.6 s) at Lively and clears; Calm and
   Off clear it silently.
6. Growth is written to `<data dir>/pet.json` shortly after each change,
   through a temporary file and a rename so a crash cannot cut it short;
   saves over 64 KB are refused. Its name, look and roaming are settings
   (`[ui.pet]`). Its card (double-click or right-click it, the palette, or
   **Settings → Pet**) shows the level ring, XP, streak, counts, what it
   wears and its tricks.

**For developers**
- Pure rules, each with vitest tests, in [`lib/domain/pet/`](../../crates/z-engine-gui/ui/src/lib/domain/pet/):
  `behavior.ts` (`petBehavior()`, `NAP_MS`, `WANDER_PERCHES`), `perches.ts`
  (`PerchId`, `PERCH_SIZE`, `nearestPerch()`, `STAGE_PERCHES`), `pose.ts`
  (`petPose()`, the moods), `emotions.ts` (how each mood looks),
  `motion.ts` (walks, hops, flings and drops), `physics.ts` and `drag.ts`
  (springs, the swing from your grip), `growth.ts` (`applyGrowth()`,
  `xpForLevel()`) and `looks.ts` (looks, stages, accessories, tricks).
- [`components/pet/`](../../crates/z-engine-gui/ui/src/components/pet/):
  `IslandPet`, `PetLayer` (the roaming pet above the app: drag, throw,
  boop), `petMotion.svelte.ts` (plays moves frame by frame), `petEyes` and
  `petGaze`, `petIdle.svelte.ts` (strolls, wanders, tricks), `PetCard` with
  `PetCardPopover`, `PetSprite` (helpers) and `PetLookPicker`; the
  components that draw it are [below](#how-it-is-drawn). An element
  becomes a perch with the `perch` action in `lib/ui/perch.svelte.ts`;
  UI-only state (card, perch, reactions) is `petUi` in
  `lib/stores/pet.svelte.ts`.
- Growth: `eventEffects()` in `lib/domain/sessions.ts` returns a
  `petGrowth` effect, `lib/runtime/effects.ts` hands it to `pet` in
  [`lib/runtime/pet.svelte.ts`](../../crates/z-engine-gui/ui/src/lib/runtime/pet.svelte.ts),
  which loads and saves through `pet_load` and `pet_save`
  ([`commands/pet.rs`](../../crates/z-engine-gui/src-tauri/src/commands/pet.rs),
  stored by `PetStore` in [`src-tauri/src/pet.rs`](../../crates/z-engine-gui/src-tauri/src/pet.rs)).
  Settings types: `PetSettings`, `PetLook` in
  [`settings/ui.rs`](../../crates/z-engine-config/src/settings/ui.rs).

## How it is drawn

**In plain words.** The pet is a small 3D figure, like a glossy toy on your
desk: it turns its body toward where it is going, and its spin trick is a
full turn. Like a toy, it costs nothing while it just sits there.

**How it works**
1. It is drawn with *WebGL*, the webview's way of drawing 3D on the
   graphics card. Its body, glossy eyes, sweat drop, props, what it wears
   and the helper orbs are 3D models; the rest of its face (drawn eyes,
   brows, mouth, cheeks, tears) is painted on a layer that slides over the
   curved body as it looks around. To face its way it turns up to about
   30° to either side, where a flat drawing would flip.
2. Every pet on screen (the island, the roaming pet, its card,
   **Settings → Pet**, first-run setup) is drawn by one shared, hidden 3D
   surface and copied into its own spot, so more pets do not use up the
   webview's few graphics contexts.
3. One frame loop draws them, and only while something plays: up to 60
   frames a second while the pet moves, blinks or changes expression, 30
   while a mood loops or helpers orbit. Breathing is a CSS animation on the
   element around the picture, so a pet that only breathes draws no
   frames. The loop stops while the window is hidden; under Reduce Motion
   it draws one still frame per change.
4. The 3D part loads on first use, as its own file. Until it is ready,
   where the webview cannot draw 3D, and while the graphics card has
   dropped the drawing surface (until it comes back), the flat drawing of
   the pet stands in, with the same moods, props and moves. The pet on the
   boot splash and the helper sprites on agent cards stay flat.

**For developers**
- [`components/pet/Pet.svelte`](../../crates/z-engine-gui/ui/src/components/pet/Pet.svelte)
  is what every screen draws: `loadPet3D()` in `petView.ts` imports
  `Pet3D.svelte` and `lib/pet3d` with a dynamic `import()` (so `three` is a
  separate chunk), and `PetFlat` (the SVG pet with `PetFace`, `PetProps`,
  `PetAccessory` and `PetBubble`) shows until then, without WebGL, or
  while `webgl.lost`. `Pet3D` draws the scene into a 2D canvas 1.5 times
  its box, over `PetRing` (plan progress) and under `PetBubble`, inside
  the same `.pet-breath` wrapper (`styles/pet-3d.css`). `PetLayer` passes
  the motion engine's `turn`, `lift` and `bob`.
- [`lib/pet3d/`](../../crates/z-engine-gui/ui/src/lib/pet3d/) is the only
  code that imports `three`: `renderer.svelte.ts` (the one offscreen
  `WebGLRenderer`; `renderView()` copies each view into its canvas with
  `drawImage`; device pixel ratio capped at 2; context loss and restore as
  `webgl`), `scheduler.ts` (`petFrames`, one `requestAnimationFrame` loop
  over each view's `rate()`), `palette.ts` (the `--pet-*` tokens as
  colors) and the scene: `petScene.ts` (lights, camera, `full` or
  `portrait` framing, `update()`, `rate()`) over `body`, `eyes`, `face`,
  `faceTexture`, `props`, `propRig`, `accessories`, `helpers`,
  `materials`, `shapes` and `tween`. Node tests build the models and check
  the frame rates without a GPU.
- Pure helpers in `lib/domain/pet/`: `keyframes.ts` (CSS easings and
  keyframe tracks), `rig3d.ts` (`bodyPose()` plays each CSS body motion and
  routine as a 3D pose; `animates()`, `faceYaw()`), `faceShapes.ts` (face
  path data shared by `PetFace` and the face texture) and `faceLayout.ts`
  (how each face part is posed).

## The island's portrait

**In plain words.** While the pet is out roaming, the island keeps a small
round picture of it, like a photo on the desk of someone who stepped out,
except this one turns to look at where the pet went.

**How it works**
- At **Lively** with roaming on, while the pet is away from the island its
  slot shows a round window about 22 pixels across with the pet's head in
  it, wearing the island's mood, look, stage and accessory. The island
  keeps its width.
- Its head turns toward the roaming pet (up to 20° to the side and 12° up
  or down, less on each when it looks diagonally) and its eyes follow it.
  The window follows its body, so a slumped, sunk or sleeping pet still
  fills it.
- A thin ring around it takes the status tone: it breathes while the agent
  works and turns amber when something needs you.
- Under Reduce Motion it holds still, turned toward the pet's side. With
  the flat pet it is the flat pet cropped to its face. **Calm** keeps the
  pet itself in the island, and **Off** still shows the dot.

**For developers**
- `IslandPet` shows [`IslandPortrait.svelte`](../../crates/z-engine-gui/ui/src/components/pet/IslandPortrait.svelte)
  while `petUi.docked` is false. `PetLayer` publishes the roaming pet's
  middle in viewport pixels as `petUi.at` (null while docked), and
  `portraitLook()` in [`lib/domain/pet/portrait.ts`](../../crates/z-engine-gui/ui/src/lib/domain/pet/portrait.ts)
  turns it into the head's yaw and pitch (eased by a `Spring`) and the
  eyes' `lookAt`.
- It draws `Pet` with `framing="portrait"` (`PORTRAIT_FRAME`), without
  helpers, the progress ring or bubbles. The slot, the portrait's ring and
  the Off dot are styled in `styles/island-pet.css`.

See also: [The desktop app's screens](features-desktop-screens.md) ·
[Everyday use: your pet](../user-guide/02-everyday-use.md#your-pet) ·
[GUI surfaces](../design/gui-surfaces.md) ·
[Settings reference](../user-guide/12-settings-reference.md) ·
[How HyperForm works](README.md)
