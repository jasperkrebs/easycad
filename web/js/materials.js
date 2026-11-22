// Printed plastics for the render: each part's material, a plastic with its own surface
// (roughness, a gloss coat, sheen, the silk's metallic shine), refractive index and how far
// light goes in it before it takes its colour. The colour and how solid the part is come from the
// custom colours; the plastic and its overrides are kept beside them, per part.
//
// Pure (no three.js), so the node tests can check it. Numbers as three's MeshPhysicalMaterial
// takes them (0..1), the depth in mm.

/**
 * The plastics, from datasheets and how prints look (indices: PLA 1.46, PETG 1.57, PC 1.585,
 * PP 1.49, ABS 1.53): `roughness` (0 glossy to 1 matte, also how much light passing through
 * spreads: printed walls frost it), `coat` (a clear gloss layer, as silk and clear plastics
 * have), `sheen` (a soft velvet edge, matte and filled plastics), `metal` (the silk's metallic
 * shine), `ior` and `depth` (mm: how far light travels in it before it's the plastic's colour).
 */
export const PLASTICS = {
  pla: { label: 'PLA', roughness: 0.4, coat: 0, sheen: 0, metal: 0, ior: 1.46, depth: 4 },
  'pla-matte': { label: 'PLA matte', roughness: 0.7, coat: 0, sheen: 0.3, metal: 0, ior: 1.46, depth: 3 },
  'pla-silk': { label: 'PLA silk', roughness: 0.22, coat: 0.6, sheen: 0, metal: 0.45, ior: 1.46, depth: 2 },
  petg: { label: 'PETG', roughness: 0.18, coat: 0.3, sheen: 0, metal: 0, ior: 1.57, depth: 12 },
  'petg-frosted': { label: 'PETG frosted', roughness: 0.55, coat: 0, sheen: 0, metal: 0, ior: 1.57, depth: 8 },
  pc: { label: 'PC clear', roughness: 0.06, coat: 0.6, sheen: 0, metal: 0, ior: 1.585, depth: 40 },
  pp: { label: 'PP', roughness: 0.45, coat: 0, sheen: 0.2, metal: 0, ior: 1.49, depth: 6 },
  abs: { label: 'ABS / ASA', roughness: 0.5, coat: 0, sheen: 0, metal: 0, ior: 1.53, depth: 3 },
  tpu: { label: 'TPU', roughness: 0.55, coat: 0, sheen: 0.4, metal: 0, ior: 1.5, depth: 5 },
  wood: { label: 'Wood fill', roughness: 0.85, coat: 0, sheen: 0.35, metal: 0, ior: 1.5, depth: 1 },
  stone: { label: 'Stone / marble fill', roughness: 0.6, coat: 0.1, sheen: 0, metal: 0, ior: 1.5, depth: 1.5 },
};

/** The properties a part can override, with their ranges. */
export const PROPERTIES = {
  roughness: [0, 1],
  coat: [0, 1],
  sheen: [0, 1],
  metal: [0, 1],
  ior: [1.3, 1.7],
  depth: [0.5, 100],
};

/**
 * A part's material from anything (a file, storage): a known plastic and only the overrides that
 * are numbers in range. null for nothing usable.
 * @param {any} raw
 * @returns {{ plastic: string } & Partial<Record<keyof typeof PROPERTIES, number>>}
 */
export function cleanMaterial(raw) {
  const r = raw && typeof raw === 'object' ? raw : {};
  const plastic = Object.hasOwn(PLASTICS, String(r.plastic)) ? String(r.plastic) : 'pla';
  /** @type {any} */
  const out = { plastic };
  for (const [key, [lo, hi]] of Object.entries(PROPERTIES)) {
    const v = Number(r[key]);
    if (r[key] !== undefined && r[key] !== null && Number.isFinite(v)) out[key] = Math.min(hi, Math.max(lo, v));
  }
  return out;
}

/**
 * A part's material as the render builds it: the plastic's values, overridden by the part's own.
 * @param {any} material
 */
export function materialValues(material) {
  const m = cleanMaterial(material);
  const base = PLASTICS[m.plastic];
  return {
    plastic: m.plastic,
    roughness: m.roughness ?? base.roughness,
    coat: m.coat ?? base.coat,
    sheen: m.sheen ?? base.sheen,
    metal: m.metal ?? base.metal,
    ior: m.ior ?? base.ior,
    depth: m.depth ?? base.depth,
  };
}

/**
 * What a part's look (custom colour, how solid, material) becomes on three's physical material:
 * the light it lets through (1 − solid), its surface, and the tint over `depth` (in metres for
 * the scene). Opaque: no tint inside (the light never gets there).
 * @param {{ color: string, opacity: number, material?: any }} look
 */
export function physicalOf(look) {
  const v = materialValues(look.material);
  const clear = Math.min(1, Math.max(0, 1 - Number(look.opacity ?? 1)));
  return {
    color: /^#[0-9a-f]{6}$/i.test(String(look.color)) ? String(look.color) : '#f2f2f2',
    transmission: clear,
    roughness: v.roughness,
    metalness: v.metal,
    ior: v.ior,
    clearcoat: v.coat,
    clearcoatRoughness: 0.08,
    sheen: v.sheen,
    sheenRoughness: 0.6,
    thickness: clear > 0 ? 1 : 0,
    attenuationDistance: clear > 0 ? v.depth * 0.001 : Infinity,
  };
}
