import { CanvasTexture, SRGBColorSpace, type Color } from "three";
import type { MouthShape } from "../domain/pet/emotions";
import { STROKE, type BrowLayout, type Drip } from "../domain/pet/faceLayout";
import { BROWS, CHEEKS, EYES, EYE_GLYPHS, EYE_LINES, MOUTHS, TEARS, type DrawnEye, type MouthPaint, type Side } from "../domain/pet/faceShapes";
import type { Point } from "../domain/pet/physics";

/**
 * The drawn parts of the face, painted with Canvas2D into a texture the
 * face shell wraps over the body: cheeks, brows, drawn eyes (happy,
 * closed, star, heart, spiral), the mouth and tears, from the same path
 * data as the SVG face, in its 32-unit square (y down).
 */

const SIDES: readonly Side[] = ["left", "right"];

/** A drawn eye's pop-in, pulse and spin, about its own centre. */
export interface Glyph {
  scale: number;
  opacity: number;
  rot: number;
}

export interface FacePaint {
  cheeks: { opacity: number; sx: number; sy: number };
  brows: BrowLayout;
  /** Null for ellipse eyes, which are 3D beads (eyes.ts). */
  eyes: ({ shape: DrawnEye } & Record<Side, Glyph>) | null;
  mouth: { shape: Exclude<MouthShape, "none">; pop: number; opacity: number; sx: number; sy: number; pivot: Point } | null;
  tears: Record<Side, Drip> | null;
}

export type FaceInk = Record<"eye" | "blush" | "mouth" | "gold" | "heart" | "sweat", Color>;

/** A two-line stroke across both eyes pops in about the middle between them. */
const LINES_CENTRE = { x: 16, y: 18.6 };
const MOUTH_CENTRE = { x: 16, y: 23.2 };

const paths = new Map<string, Path2D>();
function path(d: string): Path2D {
  let p = paths.get(d);
  if (!p) paths.set(d, (p = new Path2D(d)));
  return p;
}

function css(color: Color, alpha = 1): string {
  const { r, g, b } = color.getRGB({ r: 0, g: 0, b: 0 }, SRGBColorSpace);
  const byte = (v: number) => Math.round(Math.min(1, Math.max(0, v)) * 255);
  return `rgba(${byte(r)}, ${byte(g)}, ${byte(b)}, ${alpha})`;
}

/** Scales (and turns) what follows about (x, y). */
function about(ctx: CanvasRenderingContext2D, x: number, y: number, sx: number, sy: number, rot = 0) {
  ctx.translate(x, y);
  if (rot) ctx.rotate(rot);
  ctx.scale(sx, sy);
  ctx.translate(-x, -y);
}

function cheeks(ctx: CanvasRenderingContext2D, paint: FacePaint, ink: FaceInk) {
  const { opacity, sx, sy } = paint.cheeks;
  if (opacity <= 0) return;
  for (const side of SIDES) {
    const c = CHEEKS[side];
    ctx.save();
    ctx.translate(c.cx, c.cy);
    ctx.scale(c.rx * sx, c.ry * sy);
    // A soft-edged blush reads better on a lit, curved cheek than the SVG's hard ellipse.
    const glow = ctx.createRadialGradient(0, 0, 0, 0, 0, 1.3);
    glow.addColorStop(0, css(ink.blush, opacity));
    glow.addColorStop(0.62, css(ink.blush, opacity * 0.85));
    glow.addColorStop(1, css(ink.blush, 0));
    ctx.fillStyle = glow;
    ctx.beginPath();
    ctx.arc(0, 0, 1.3, 0, Math.PI * 2);
    ctx.fill();
    ctx.restore();
  }
}

function brows(ctx: CanvasRenderingContext2D, paint: FacePaint, ink: FaceInk) {
  if (paint.brows.opacity <= 0) return;
  ctx.strokeStyle = css(ink.eye, paint.brows.opacity);
  ctx.lineWidth = STROKE.brow;
  for (const side of SIDES) {
    const brow = BROWS[side];
    const pose = paint.brows[side];
    ctx.save();
    ctx.translate(pose.dx, pose.dy);
    about(ctx, brow.cx, brow.cy, 1, 1, pose.rot);
    ctx.stroke(path(brow.d));
    ctx.restore();
  }
}

function drawnEyes(ctx: CanvasRenderingContext2D, paint: FacePaint, ink: FaceInk) {
  const eyes = paint.eyes;
  if (!eyes) return;
  const { shape } = eyes;
  if (shape === "happy" || shape === "closed") {
    const glyph = eyes.left;
    ctx.save();
    ctx.globalAlpha = glyph.opacity;
    about(ctx, LINES_CENTRE.x, LINES_CENTRE.y, glyph.scale, glyph.scale);
    ctx.strokeStyle = css(ink.eye);
    ctx.lineWidth = STROKE.eyeLine;
    ctx.stroke(path(EYE_LINES[shape]));
    ctx.restore();
    return;
  }
  for (const side of SIDES) {
    const glyph = eyes[side];
    ctx.save();
    ctx.globalAlpha = glyph.opacity;
    ctx.translate(EYES[side].cx, EYES[side].cy);
    ctx.rotate(glyph.rot);
    ctx.scale(glyph.scale, glyph.scale);
    if (shape === "spiral") {
      ctx.strokeStyle = css(ink.eye);
      ctx.lineWidth = STROKE.spiral;
      ctx.stroke(path(EYE_GLYPHS.spiral));
    } else {
      ctx.fillStyle = css(shape === "star" ? ink.gold : ink.heart);
      ctx.fill(path(EYE_GLYPHS[shape]));
    }
    ctx.restore();
  }
}

function mouth(ctx: CanvasRenderingContext2D, paint: FacePaint, ink: FaceInk) {
  const m = paint.mouth;
  if (!m || m.opacity <= 0) return;
  ctx.save();
  ctx.globalAlpha = m.opacity;
  about(ctx, MOUTH_CENTRE.x, MOUTH_CENTRE.y, m.pop, m.pop);
  about(ctx, m.pivot.x, m.pivot.y, m.sx, m.sy);
  const fills: Record<MouthPaint, string | null> = { line: null, thin: null, fill: css(ink.mouth), tongue: css(ink.blush) };
  for (const part of MOUTHS[m.shape]) {
    const fill = fills[part.paint];
    const shape = part.kind === "path" ? path(part.d) : null;
    if (part.kind === "ellipse") {
      ctx.beginPath();
      ctx.ellipse(part.cx, part.cy, part.rx, part.ry, 0, 0, Math.PI * 2);
    }
    if (fill) {
      ctx.fillStyle = fill;
      if (shape) ctx.fill(shape);
      else ctx.fill();
    } else {
      ctx.strokeStyle = css(ink.eye);
      ctx.lineWidth = part.paint === "thin" ? STROKE.mouthThin : STROKE.mouth;
      if (shape) ctx.stroke(shape);
      else ctx.stroke();
    }
  }
  ctx.restore();
}

function tears(ctx: CanvasRenderingContext2D, paint: FacePaint, ink: FaceInk) {
  if (!paint.tears) return;
  for (const side of SIDES) {
    const drip = paint.tears[side];
    if (drip.opacity <= 0) continue;
    ctx.save();
    ctx.fillStyle = css(ink.sweat, drip.opacity);
    ctx.translate(0, drip.dy);
    ctx.fill(path(TEARS[side]));
    ctx.restore();
  }
}

/** Paints the face into `ctx`, `px` pixels square, back to front. */
export function paintFace(ctx: CanvasRenderingContext2D, px: number, paint: FacePaint, ink: FaceInk) {
  ctx.setTransform(px / 32, 0, 0, px / 32, 0, 0);
  ctx.clearRect(0, 0, 32, 32);
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  cheeks(ctx, paint, ink);
  brows(ctx, paint, ink);
  drawnEyes(ctx, paint, ink);
  mouth(ctx, paint, ink);
  tears(ctx, paint, ink);
}

/** A key that changes whenever the painted face would, so an unchanged face is not painted and uploaded again. */
export function paintKey(paint: FacePaint): string {
  return JSON.stringify(paint, (_, value: unknown) => (typeof value === "number" ? Math.round(value * 500) / 500 : value));
}

/**
 * The face's texture: a canvas `px` square, repainted only when the face
 * changes. Without a document (tests) there is no canvas, and no texture.
 */
export class FaceTexture {
  readonly texture: CanvasTexture | null = null;
  #ctx: CanvasRenderingContext2D | null = null;
  #px: number;
  #key = "";

  constructor(px = 256) {
    this.#px = px;
    if (typeof document === "undefined") return;
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = px;
    this.#ctx = canvas.getContext("2d");
    if (!this.#ctx) return;
    this.texture = new CanvasTexture(canvas);
    this.texture.colorSpace = SRGBColorSpace;
    this.texture.anisotropy = 4;
  }

  /** Paints the face unless it shows exactly this already; returns whether it painted. */
  paint(paint: FacePaint, ink: FaceInk): boolean {
    const key = paintKey(paint);
    if (!this.#ctx || !this.texture || key === this.#key) return false;
    this.#key = key;
    paintFace(this.#ctx, this.#px, paint, ink);
    this.texture.needsUpdate = true;
    return true;
  }
}
