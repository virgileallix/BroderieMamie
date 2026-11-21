// Dessin d'une broderie dans un <canvas>, avec zoom, déplacement, cadre et animation point par point.

const HOOPS = [[100, 100], [130, 180], [160, 260], [200, 200], [200, 300], [240, 150], [360, 200]];

/** Le plus petit cadre courant dans lequel le motif tient (en mm), ou null. */
export function bestHoop(wMM, hMM) {
  return HOOPS
    .filter(([a, b]) => (wMM <= a && hMM <= b) || (wMM <= b && hMM <= a))
    .sort((p, q) => p[0] * p[1] - q[0] * q[1])[0] || null;
}

function shade(hex, f) {
  const n = parseInt(hex.slice(1), 16);
  const c = s => Math.max(0, Math.min(255, Math.round(((n >> s) & 255) * f)));
  return `rgb(${c(16)},${c(8)},${c(0)})`;
}

export class Viewer {
  constructor(canvas) {
    this.cv = canvas;
    this.ctx = canvas.getContext("2d");
    this.design = null;
    this.zoom = 1;
    this.panX = 0;
    this.panY = 0;
    this.limit = Infinity; // nombre de points dessinés (animation)
    this.showHoop = false;
    this.onCurrentBlock = () => {};
    this.#bindPointer();
    new ResizeObserver(() => this.draw()).observe(canvas);
  }

  set(design) {
    this.design = design;
    this.reset();
  }

  reset() {
    this.zoom = 1; this.panX = 0; this.panY = 0; this.limit = Infinity;
    this.draw();
  }

  zoomBy(f, cx, cy) {
    const old = this.zoom;
    this.zoom = Math.max(0.3, Math.min(40, this.zoom * f));
    // Zoom centré sur le point visé.
    if (cx != null) {
      const k = this.zoom / old;
      this.panX = cx - (cx - this.panX) * k;
      this.panY = cy - (cy - this.panY) * k;
    } else {
      this.panX *= this.zoom / old; this.panY *= this.zoom / old;
    }
    this.draw();
  }

  #bindPointer() {
    const cv = this.cv;
    let drag = null;
    cv.addEventListener("pointerdown", e => {
      drag = { x: e.clientX, y: e.clientY, px: this.panX, py: this.panY };
      cv.setPointerCapture(e.pointerId);
    });
    cv.addEventListener("pointermove", e => {
      if (!drag) return;
      this.panX = drag.px + (e.clientX - drag.x);
      this.panY = drag.py + (e.clientY - drag.y);
      this.draw();
    });
    const end = () => { drag = null; };
    cv.addEventListener("pointerup", end);
    cv.addEventListener("pointercancel", end);
    cv.addEventListener("wheel", e => {
      e.preventDefault();
      const r = cv.getBoundingClientRect();
      this.zoomBy(e.deltaY < 0 ? 1.2 : 1 / 1.2, e.clientX - r.left - r.width / 2, e.clientY - r.top - r.height / 2);
    }, { passive: false });
    cv.addEventListener("dblclick", () => this.reset());
  }

  draw() {
    const cv = this.cv, ctx = this.ctx;
    const dpr = window.devicePixelRatio || 1;
    const W = cv.clientWidth, H = cv.clientHeight;
    if (!W || !H) return;
    if (cv.width !== Math.round(W * dpr) || cv.height !== Math.round(H * dpr)) {
      cv.width = Math.round(W * dpr); cv.height = Math.round(H * dpr);
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, W, H);
    const d = this.design;
    if (!d) return;

    const pad = Math.min(W, H) * 0.08;
    let spanX = d.maxX - d.minX, spanY = d.maxY - d.minY;
    const hoop = this.showHoop ? this.#hoopFor(d) : null;
    if (hoop) { spanX = Math.max(spanX, hoop[0] * 10); spanY = Math.max(spanY, hoop[1] * 10); }
    const fit = Math.min((W - 2 * pad) / Math.max(1, spanX), (H - 2 * pad) / Math.max(1, spanY));
    const s = fit * this.zoom;
    const cx = (d.minX + d.maxX) / 2, cy = (d.minY + d.maxY) / 2;
    const ox = W / 2 - cx * s + this.panX, oy = H / 2 - cy * s + this.panY;

    if (hoop) this.#drawHoop(hoop, s, W / 2 + this.panX, H / 2 + this.panY);

    const lw = Math.max(0.7, Math.min(7, 3.6 * s)); // fil ≈ 0,36 mm
    ctx.lineCap = "round"; ctx.lineJoin = "round";
    let remaining = this.limit;
    let needle = null, current = d.blocks.length - 1;
    for (let bi = 0; bi < d.blocks.length; bi++) {
      if (remaining <= 0) break;
      const b = d.blocks[bi], p = b.pts;
      const path = new Path2D();
      let started = false;
      for (let i = 0; i < p.length; i += 3) {
        const X = p[i] * s + ox, Y = p[i + 1] * s + oy;
        if (p[i + 2] === 1 || !started) { path.moveTo(X, Y); started = true; }
        else {
          path.lineTo(X, Y);
          if (--remaining <= 0) { needle = [X, Y]; current = bi; break; }
        }
      }
      ctx.strokeStyle = shade(b.color, 0.7); ctx.lineWidth = lw * 1.3; ctx.stroke(path);
      ctx.strokeStyle = b.color; ctx.lineWidth = lw; ctx.stroke(path);
      if (lw > 1.6) { ctx.strokeStyle = "rgba(255,255,255,.22)"; ctx.lineWidth = lw * 0.35; ctx.stroke(path); }
    }
    if (needle) {
      ctx.save();
      ctx.shadowColor = "rgba(0,0,0,.35)"; ctx.shadowBlur = 6;
      ctx.fillStyle = "#fff"; ctx.beginPath(); ctx.arc(needle[0], needle[1], 9, 0, 7); ctx.fill();
      ctx.restore();
      ctx.fillStyle = "#c8566e"; ctx.beginPath(); ctx.arc(needle[0], needle[1], 6, 0, 7); ctx.fill();
    }
    this.onCurrentBlock(this.limit === Infinity ? -1 : current);
  }

  #hoopFor(d) {
    const w = (d.maxX - d.minX) / 10, h = (d.maxY - d.minY) / 10;
    const hp = bestHoop(w, h);
    if (!hp) return null;
    let [a, b] = hp;
    if (w > a || h > b) [a, b] = [b, a];
    return [a, b];
  }

  #drawHoop([a, b], s, cx, cy) {
    const ctx = this.ctx;
    const w = a * 10 * s, h = b * 10 * s, r = Math.min(w, h) * 0.08;
    ctx.save();
    ctx.lineWidth = 2; ctx.setLineDash([10, 8]); ctx.strokeStyle = "rgba(63,107,87,.75)";
    ctx.beginPath(); ctx.roundRect(cx - w / 2, cy - h / 2, w, h, r); ctx.stroke();
    ctx.setLineDash([]); ctx.strokeStyle = "rgba(63,107,87,.5)";
    ctx.beginPath(); ctx.moveTo(cx - 14, cy); ctx.lineTo(cx + 14, cy); ctx.moveTo(cx, cy - 14); ctx.lineTo(cx, cy + 14); ctx.stroke();
    ctx.fillStyle = "rgba(63,107,87,.9)"; ctx.font = "800 14px Nunito, sans-serif";
    ctx.fillText(`Cadre ${a / 10} × ${b / 10} cm`, cx - w / 2 + 12, cy - h / 2 + 24);
    ctx.restore();
  }
}
