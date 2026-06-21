/**
 * Drox splash — spec portée depuis drox-tui/src/ui/boot_splash.rs
 *
 * Usage dans le fork VS Code Drox :
 * - Importer dans une webview de démarrage, un workbench part custom, ou une extension `drox.splash`
 * - Appeler `createDroxSplashController(domRoot)` au lancement de la fenêtre
 *
 * Référence TUI : docs/animation-start/README.md
 */

export interface DroxSplashTimeline {
  appear: number;
  exit: number;
  glow: number;
}

export interface DroxSplashConfig {
  frames: number;
  frameMs: number;
  appearEnd: number;
  holdEnd: number;
  logoLines: string[];
  logoCompact: string;
  taglineBoot: string;
  taglineProduct: string;
}

export const DEFAULT_CONFIG: DroxSplashConfig = {
  frames: 52,
  frameMs: 50,
  appearEnd: 0.48,
  holdEnd: 0.68,
  logoLines: [
    '██████╗ ██████╗  ██████╗ ██╗  ██╗',
    '██╔══██╗██╔══██╗██╔═══██╗╚██╗██╔╝',
    '██║  ██║██████╔╝██║   ██║ ╚███╔╝ ',
    '██║  ██║██╔══██╗██║   ██║ ██╔██╗ ',
    '██████╔╝██║  ██║╚██████╔╝██╔╝ ██╗',
    '╚═════╝ ╚═╝  ╚═╝ ╚═════╝ ╚═╝  ╚═╝',
  ],
  logoCompact: 'DROX',
  taglineBoot: 'Initializing…',
  taglineProduct: 'Local agent · terminal',
};

function easeOut(t: number): number {
  return 1 - (1 - t) ** 2;
}

function easeIn(t: number): number {
  return t * t;
}

export function timelineAt(tick: number, total: number, cfg: DroxSplashConfig): DroxSplashTimeline {
  const t = tick / Math.max(total - 1, 1);
  const appear =
    t < cfg.appearEnd ? easeOut(Math.min(1, Math.max(0, t / cfg.appearEnd))) : 1;
  const exit =
    t > cfg.holdEnd
      ? easeIn(Math.min(1, Math.max(0, (t - cfg.holdEnd) / (1 - cfg.holdEnd))))
      : 0;
  let glow: number;
  if (t < cfg.appearEnd) {
    glow = easeOut(Math.min(1, Math.max(0, t / cfg.appearEnd))) * 0.55;
  } else if (t <= cfg.holdEnd) {
    glow = 0.55 + Math.min(1, Math.max(0, (t - cfg.appearEnd) / (cfg.holdEnd - cfg.appearEnd))) * 0.25;
  } else {
    glow = 0.8 * (1 - exit);
  }
  return { appear, exit, glow };
}

function lerp(a: number, b: number, t: number): number {
  return a + (b - a) * Math.max(0, Math.min(1, t));
}

function lerpColorRgb(
  from: [number, number, number],
  to: [number, number, number],
  t: number,
): string {
  const r = Math.round(lerp(from[0], to[0], t));
  const g = Math.round(lerp(from[1], to[1], t));
  const b = Math.round(lerp(from[2], to[2], t));
  return `rgb(${r}, ${g}, ${b})`;
}

const PHOSPHOR_DIM: [number, number, number] = [0x1a, 0x3d, 0x22];
const PHOSPHOR_BRIGHT: [number, number, number] = [0x39, 0xff, 0x14];
const BG_ELEVATED: [number, number, number] = [0x0b, 0x16, 0x0c];
const SPLASH_BG = '#030704';

export interface DroxSplashController {
  stop(): void;
}

/**
 * Affiche le splash dans un conteneur DOM plein écran (position absolute / fixed).
 */
export function createDroxSplashController(
  root: HTMLElement,
  cfg: DroxSplashConfig = DEFAULT_CONFIG,
): DroxSplashController {
  root.innerHTML = '';
  root.style.position = 'fixed';
  root.style.inset = '0';
  root.style.background = SPLASH_BG;
  root.style.display = 'flex';
  root.style.flexDirection = 'column';
  root.style.alignItems = 'center';
  root.style.justifyContent = 'center';
  root.style.zIndex = '99999';
  root.style.fontFamily = 'Consolas, "Cascadia Mono", monospace';
  root.style.overflow = 'hidden';

  const halo = document.createElement('div');
  halo.style.position = 'absolute';
  halo.style.inset = '0';
  halo.style.pointerEvents = 'none';
  root.appendChild(halo);

  const logo = document.createElement('pre');
  logo.style.margin = '0';
  logo.style.textAlign = 'center';
  logo.style.lineHeight = '1.1';
  logo.style.whiteSpace = 'pre';
  logo.style.position = 'relative';
  logo.style.zIndex = '1';
  root.appendChild(logo);

  const tagline = document.createElement('div');
  tagline.style.position = 'absolute';
  tagline.style.bottom = '12%';
  tagline.style.width = '100%';
  tagline.style.textAlign = 'center';
  tagline.style.fontSize = '13px';
  tagline.style.zIndex = '1';
  root.appendChild(tagline);

  let tick = 0;
  let stopped = false;
  const useCompact = window.innerWidth < 480 || window.innerHeight < 320;

  const timer = window.setInterval(() => {
    if (stopped) return;
    const tl = timelineAt(tick, cfg.frames, cfg);
    const haloStrength =
      Math.max(0, Math.min(1, (tl.appear - 0.12) / 0.52)) * (1 - tl.exit);
    halo.style.background = `radial-gradient(ellipse 55% 45% at 50% 48%, ${lerpColorRgb(
      [0x06, 0x0e, 0x07],
      [0x10, 0x1e, 0x11],
      haloStrength * tl.glow,
    )} 0%, transparent 72%)`;

    const logoStrength =
      Math.max(0, Math.min(1, (tl.appear - 0.22) / 0.5)) * (1 - tl.exit);
    if (logoStrength > 0.02) {
      const peak = lerpColorRgb(PHOSPHOR_DIM, PHOSPHOR_BRIGHT, tl.glow * 0.65);
      if (useCompact) {
        logo.textContent = cfg.logoCompact;
        logo.style.color = lerpColorRgb(PHOSPHOR_DIM, PHOSPHOR_BRIGHT, logoStrength * tl.glow * 0.6);
        logo.style.fontWeight = '700';
      } else {
        const lines = cfg.logoLines.map((line, row) => {
          const rowReveal =
            Math.max(0, Math.min(1, logoStrength * cfg.logoLines.length - row)) * logoStrength;
          if (rowReveal <= 0) return '';
          const color =
            rowReveal >= 0.92 ? peak : lerpColorRgb(BG_ELEVATED, peak, rowReveal);
          return `<span style="color:${color};font-weight:${rowReveal >= 0.92 ? 700 : 400}">${line}</span>`;
        });
        logo.innerHTML = lines.filter(Boolean).join('\n');
      }
    } else {
      logo.textContent = '';
    }

    const tagVisible = tl.appear > 0.55 && tl.exit < 0.75;
    if (tagVisible) {
      const alpha = Math.max(0, Math.min(1, (tl.appear - 0.55) / 0.3)) * (1 - tl.exit);
      tagline.style.opacity = String(alpha * 0.85);
      tagline.style.color = lerpColorRgb([0x0b, 0x16, 0x0c], [0x12, 0x22, 0x14], alpha * 0.7);
      tagline.textContent =
        tl.appear > 0.82 && tl.exit < 0.2 ? cfg.taglineBoot : cfg.taglineProduct;
    } else {
      tagline.textContent = '';
    }

    tick += 1;
    if (tick >= cfg.frames) {
      window.clearInterval(timer);
      root.remove();
    }
  }, cfg.frameMs);

  return {
    stop() {
      stopped = true;
      window.clearInterval(timer);
      root.remove();
    },
  };
}
