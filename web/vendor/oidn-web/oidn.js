var $n = Object.defineProperty;
var bn = (n, e, t) => e in n ? $n(n, e, { enumerable: !0, configurable: !0, writable: !0, value: t }) : n[e] = t;
var m = (n, e, t) => bn(n, typeof e != "symbol" ? e + "" : e, t);
class kn {
  constructor() {
    m(this, "dims", []);
    m(this, "paddedDims", []);
    m(this, "layout", "x");
    m(this, "dataType", "Float32");
  }
  getByteSize() {
    let e = 1;
    for (const t of this.paddedDims)
      e *= t;
    return this.dataType === "Float32" ? e *= 4 : this.dataType === "Float16" && (e *= 2), e;
  }
}
class In {
  constructor(e, t) {
    this.desc = e, this.data = t;
  }
}
class Bn {
  constructor(e) {
    m(this, "offset", 0);
    this._view = e;
  }
  read(e) {
    const t = this._view, i = this.offset;
    switch (this.offset += e, e) {
      case 1:
        return t.getUint8(i);
      case 2:
        return t.getUint16(i, !0);
      case 4:
        return t.getUint32(i, !0);
      case 8:
        return Number(t.getBigUint64(i, !0));
      default:
        throw new Error("unsupported read size");
    }
  }
}
function Tn(n) {
  const e = new Uint8Array(n), t = new Bn(new DataView(n));
  if (t.read(2) !== 16855)
    throw new Error("invalid or corrupted weights blob");
  const r = t.read(1);
  if (t.read(1), r !== 2)
    throw new Error("unsupported weights blob version");
  const o = t.read(8);
  t.offset = o;
  const a = t.read(4), u = /* @__PURE__ */ new Map();
  for (let s = 0; s < a; ++s) {
    const l = new kn(), p = t.read(2), c = new TextDecoder().decode(
      e.subarray(t.offset, t.offset + p)
    );
    t.offset += p;
    const d = t.read(1);
    for (let v = 0; v < d; ++v)
      l.dims.push(t.read(4));
    l.paddedDims = [...l.dims], new TextDecoder().decode(
      e.subarray(t.offset, t.offset + d)
    ) === "oihw" && (l.layout = "oihw"), t.offset += d;
    const f = String.fromCharCode(t.read(1));
    if (f === "f")
      l.dataType = "Float32";
    else if (f === "h")
      l.dataType = "Float16";
    else
      throw new Error("invalid tensor data type");
    const g = t.read(8), y = e.slice(
      g,
      g + l.getByteSize()
    );
    u.set(c, new In(l, y));
  }
  return u;
}
function Pn(n, e) {
  return n.channels === e.channels;
}
const Ue = 8;
class ze {
  constructor(e, t, i) {
    m(this, "autoUpdateOutputBuffer", !0);
    m(this, "_label");
    m(this, "_device");
    m(this, "_outputBuffers", {});
    m(this, "_pipeline");
    m(this, "_bindGroups", []);
    m(this, "_needsUpdatePipeline", !0);
    m(this, "_needsResizeBuffer", !0);
    m(this, "_inputs", []);
    m(this, "_outputs", []);
    m(this, "_uniforms", []);
    m(this, "_uniformBuffers", {});
    m(this, "_width", 10);
    m(this, "_height", 10);
    m(this, "_execWidth");
    m(this, "_execHeight");
    m(this, "_csCode", "");
    m(this, "_csMain");
    m(this, "_csDefine");
    m(this, "_groupOffsets", {
      inputs: 0,
      uniforms: 1,
      outputs: 2
    });
    this._label = e, this._device = t, this._csMain = i.csMain, this._csDefine = i.csDefine, this._inputs = i.inputs, this._outputs = i.outputs, this._uniforms = i.uniforms, this.autoUpdateOutputBuffer = i.autoUpdateOutputBuffer ?? !0, i.uniforms.forEach((r) => {
      this._uniformBuffers[r.label] = t.createBuffer({
        label: this._label,
        size: r.data.byteLength,
        usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST
      }), this._device.queue.writeBuffer(
        this._uniformBuffers[r.label],
        0,
        r.data
      );
    });
  }
  setCSCode({ csDefine: e, csMain: t }) {
    this._csDefine = e, this._csMain = t, this._needsUpdatePipeline = !0;
  }
  setSize(e, t) {
    e = Math.ceil(e), t = Math.ceil(t);
    const i = e !== this._width || t !== this._height;
    this._width = e, this._height = t, i && (this._needsResizeBuffer = !0, this._needsUpdatePipeline = !0);
  }
  setExecuteSize(e, t) {
    e = Math.ceil(e), t = Math.ceil(t), this._execWidth = e, this._execHeight = t;
  }
  setOutputParams(e) {
    this.autoUpdateOutputBuffer && this._updateOutputBuffers(e), this._needsUpdatePipeline = !0;
  }
  setOutputBuffers(e) {
    this._outputBuffers = Object.keys(e).reduce((t, i) => (t[i] = {
      buffer: e[i],
      params: { channels: 4 }
    }, t), {});
  }
  setUniform(e, t) {
    const i = this._uniformBuffers[e];
    this._device.queue.writeBuffer(i, 0, t);
  }
  getOutput(e) {
    return this._needsResizeBuffer && this.autoUpdateOutputBuffer && (this._resizeOutputBuffers(), this._needsResizeBuffer = !1), this._outputBuffers[e].buffer;
  }
  dispose(e = !0) {
    Object.keys(this._uniformBuffers).forEach((t) => {
      this._uniformBuffers[t].destroy();
    }), e && Object.keys(this._outputBuffers).forEach((t) => {
      this._outputBuffers[t].buffer.destroy();
    });
  }
  _createBuffer(e) {
    const t = this._width * this._height * 4 * 4;
    return this._device.createBuffer({
      label: this._label,
      // webgpu needs buffer at least 80 bytes.
      size: Math.max(t, 80),
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC
    });
  }
  _resizeOutputBuffers() {
    const e = this._outputBuffers;
    for (const t in e) {
      const { buffer: i, params: r } = e[t];
      i.destroy(), e[t].buffer = this._createBuffer(r);
    }
  }
  _updateOutputBuffers(e) {
    var i, r;
    const t = this._outputBuffers;
    for (const o in e) {
      const a = e[o];
      if (!Pn(
        a,
        ((i = t[o]) == null ? void 0 : i.params) || {}
      )) {
        (r = t[o]) == null || r.buffer.destroy();
        const u = this._createBuffer(a);
        t[o] = {
          buffer: u,
          params: a
        };
      }
    }
  }
  _updatePipeline(e, t) {
    if (!this._needsUpdatePipeline)
      return;
    this._needsUpdatePipeline = !1;
    const i = this._device, r = this._getFullCs(e, t);
    r !== this._csCode && (this._csCode = r, this._pipeline = i.createComputePipeline({
      label: this._label,
      layout: "auto",
      compute: {
        module: i.createShaderModule({
          label: this._label,
          code: r
        }),
        entryPoint: "main"
      }
    }), this._updateBindGroups());
  }
  _getFullCs(e, t) {
    const i = this._inputs, r = this._uniforms;
    let o = 0;
    const a = this._groupOffsets = {
      inputs: 0,
      uniforms: 0,
      outputs: 0
    };
    return i.length > 0 && o++, r.length > 0 && (a.uniforms = o, o++), a.outputs = o, `
${i.sort().map((s, l) => {
      const p = `@group(${a.inputs}) @binding(${l}) `, c = `in_${s}`;
      return t[s] === "texture" ? `${p} var ${c}: texture_2d<f32>;` : `${p} var<storage, read> ${c}: array<vec${e[s].channels}f>;`;
    }).join(`
`)}
${this._uniforms.map(
      (s, l) => `@group(${a.uniforms}) @binding(${l}) var<uniform> ${s.label}: ${s.type};`
    ).join(`
`)}

${this._outputs.map(
      (s, l) => `@group(${a.outputs}) @binding(${l}) var<storage, read_write> out_${s}: array<vec${this._outputBuffers[s].params.channels}f>;`
    ).join(`
`)}
${this._csDefine ?? ""}
@compute @workgroup_size(${Ue}, ${Ue}, 1)
fn main(@builtin(global_invocation_id) globalId: vec3u) {
${this._csMain}
}
`;
  }
  _updateBindGroups() {
    const e = [], t = this._device, i = this._groupOffsets;
    this._uniforms.length > 0 && (e[i.uniforms] = t.createBindGroup({
      label: this._label,
      layout: this._pipeline.getBindGroupLayout(i.uniforms),
      entries: this._uniforms.map(
        (r, o) => ({
          binding: o,
          resource: {
            buffer: this._uniformBuffers[r.label]
          }
        })
      )
    })), this._bindGroups = e;
  }
  createPass(e, t) {
    this._needsResizeBuffer && this.autoUpdateOutputBuffer && (this._resizeOutputBuffers(), this._needsResizeBuffer = !1);
    const i = this._inputs.reduce((a, u) => (a[u] = t[u].buffer ? "buffer" : "texture", a), {});
    this._updatePipeline(t, i);
    const r = this._groupOffsets;
    this._inputs.length > 0 && (this._bindGroups[r.inputs] = this._device.createBindGroup({
      label: this._label,
      layout: this._pipeline.getBindGroupLayout(r.inputs),
      entries: this._inputs.map((a, u) => ({
        binding: u,
        // TODO
        resource: t[a].buffer ? {
          buffer: t[a].buffer
        } : t[a].texture.createView()
      }))
    })), this._bindGroups[r.outputs] = this._device.createBindGroup({
      label: this._label,
      layout: this._pipeline.getBindGroupLayout(r.outputs),
      entries: this._outputs.map((a, u) => ({
        binding: u,
        resource: {
          buffer: this._outputBuffers[a].buffer
        }
      }))
    });
    const o = e.beginComputePass();
    o.setPipeline(this._pipeline), this._bindGroups.forEach((a, u) => {
      o.setBindGroup(u, a);
    }), o.dispatchWorkgroups(
      Math.ceil((this._execWidth ?? this._width) / Ue),
      Math.ceil((this._execHeight ?? this._height) / Ue),
      1
    ), o.end();
  }
}
const hi = 1412.83765, fi = 1.64593172, gi = 0.431384981, mi = -0.00294139609, _i = 0.192653254, yi = 0.00626026094, wi = 0.998620152, Cn = 15794576e-13, Sn = 0.0322087631, En = 0.00223151711, An = 0.370974749, vi = 65504, xi = bi(vi), Mn = 1 / xi, On = xi, $i = Math.log(vi + 1), Nn = 1 / $i;
function bi(n) {
  return n <= Cn ? hi * n : n <= Sn ? fi * Math.pow(n, gi) + mi : _i * Math.log(n + yi) + wi;
}
function Un(n) {
  return n <= En ? n / hi : n <= An ? Math.pow((n - mi) / fi, 1 / gi) : Math.exp((n - wi) / _i) - yi;
}
function zn(n, e) {
  return e === "log" ? Math.log(n + 1) * Nn : bi(n) * Mn;
}
function Ln(n, e) {
  return e === "log" ? Math.exp(n * $i) - 1 : Un(n * On);
}
function Wn({
  data: n,
  channels: e,
  inputScale: t,
  transfer: i = "pu"
}) {
  const r = new Float32Array(n);
  for (let o = 0; o < r.length; o += e)
    for (let a = 0; a < 3; a++)
      r[o + a] = zn(
        r[o + a] * t,
        i
      );
  return r;
}
function Dn({
  data: n,
  channels: e,
  inputScale: t,
  transfer: i = "pu"
}) {
  const r = new Float32Array(n), o = 1 / t;
  for (let a = 0; a < r.length; a += e)
    for (let u = 0; u < 3; u++)
      r[a + u] = Ln(r[a + u], i) * o;
  return r;
}
const ki = 1412.83765, Ii = 1.64593172, Bi = 0.431384981, Ti = -0.00294139609, Pi = 0.192653254, Ci = 0.00626026094, Si = 0.998620152, Ei = 15794576e-13, Ai = 0.0322087631, Rn = 0.00223151711, Yn = 0.370974749;
function Gn(n) {
  return n <= Ei ? n = ki * n : n <= Ai ? n = Ii * Math.pow(n, Bi) + Ti : n = Pi * Math.log(n + Ci) + Si, n;
}
const Mi = 65504, Oi = Gn(Mi), Xn = 1 / Oi, Fn = Oi, mt = Math.log(Mi + 1), Vn = 1 / mt;
class ct {
  constructor(e, t, i, r) {
    this.x = e, this.y = t, this.width = i, this.height = r;
  }
}
function Hn({
  data: n,
  channels: e
}) {
  let t = 0;
  for (let a = 0; a < n.length; a += e) {
    const u = n[a], s = n[a + 1], l = n[a + 2], p = 0.212671 * u + 0.71516 * s + 0.072169 * l;
    t += Math.log2(p + 1e-4);
  }
  const i = n.length / e, r = t / i;
  return 0.18 / Math.pow(2, r);
}
const Lt = `
const a = ${ki};
const b = ${Ii};
const c = ${Bi};
const d = ${Ti};
const e = ${Pi};
const f = ${Ci};
const g = ${Si};
const y0 =${Ei};
const y1 =${Ai};
const x0 =${Rn};
const x1 =${Yn};

const normScale = ${Xn};
const rcpNormScale = ${Fn};
`;
class jn {
  constructor(e, t, i = "pu") {
    m(this, "_inputPassAux");
    m(this, "_inputPassColor");
    m(this, "_outputPass");
    m(this, "_copyPass");
    m(this, "_isInputTexture");
    this._device = e, this._isHDR = t, this._hdrTransfer = i;
    const r = [
      {
        label: "inputScale",
        type: "f32",
        data: new Float32Array([1])
      },
      {
        label: "inputSize",
        type: "vec2i",
        data: new Int32Array(2)
      },
      {
        label: "outputSize",
        type: "vec2i",
        data: new Int32Array(2)
      },
      {
        label: "inputOffset",
        type: "vec2i",
        data: new Int32Array(2)
      }
    ];
    this._inputPassAux = new ze("inputPassAux", this._device, {
      inputs: ["color", "albedo", "normal"],
      outputs: ["color", "albedo", "normal"],
      uniforms: r,
      csDefine: "",
      csMain: ""
    }), this._inputPassColor = new ze("inputPassColor", this._device, {
      inputs: ["color"],
      outputs: ["color"],
      uniforms: r,
      csDefine: "",
      csMain: ""
    }), this._outputPass = new ze("outputPass", this._device, {
      inputs: ["color", "raw"],
      outputs: ["color"],
      uniforms: [
        {
          label: "inputScale",
          type: "f32",
          data: new Float32Array([1])
        },
        {
          label: "inputSize",
          type: "vec2i",
          data: new Int32Array(2)
        },
        {
          label: "outputSize",
          type: "vec2i",
          data: new Int32Array(2)
        },
        {
          label: "imageSize",
          type: "vec2i",
          data: new Int32Array(2)
        },
        {
          label: "inputOffset",
          type: "vec2i",
          data: new Int32Array(2)
        },
        {
          label: "outputOffset",
          type: "vec2i",
          data: new Int32Array(2)
        }
      ],
      csDefine: "",
      csMain: ""
    }), this._copyPass = new ze("copyPass", this._device, {
      inputs: ["color"],
      outputs: ["color"],
      autoUpdateOutputBuffer: !1,
      uniforms: [
        {
          label: "size",
          type: "vec2i",
          data: new Int32Array(2)
        }
      ],
      csMain: (
        /*wgsl*/
        `
let outIdx = i32(globalId.x + globalId.y * u32(size.x));
out_color[outIdx] = textureLoad(in_color, globalId.xy, 0);
`
      )
    }), this._inputPassAux.setOutputParams({
      color: { channels: 3 },
      albedo: { channels: 3 },
      normal: { channels: 3 }
    }), this._inputPassColor.setOutputParams({
      color: { channels: 3 }
    }), this._outputPass.setOutputParams({
      color: { channels: 4 }
    });
  }
  _updatePasses(e, t = !1) {
    if (this._isInputTexture != null && this._isInputTexture === e)
      return;
    this._isInputTexture = e;
    const i = this._isHDR, r = this._hdrTransfer === "log" ? "fn HDRForward(y: f32) -> f32 { return log(y + 1.0) * logNormScale; }" : `fn HDRForward(y: f32) -> f32 {
  if (y <= y0) { return a * y * normScale; }
  else if (y <= y1) { return (b * pow(y, c) + d) * normScale; }
  else { return (e * log(y + f) + g) * normScale; }
}`, o = this._hdrTransfer === "log" ? "fn HDRInverse(x: f32) -> f32 { return exp(x * logXMax) - 1.0; }" : `fn HDRInverse(x: f32) -> f32 {
  let y = x * rcpNormScale;
  if (y <= x0) { return y / a; }
  else if (y <= x1) { return pow((y - d) / b, 1 / c); }
  else { return exp((y - g) / e) - f; }
}`, a = (
      /* wgsl */
      `
${Lt}
const logXMax = ${mt};
const logNormScale = ${Vn};
${r}`
    );
    function u(l) {
      return e ? `textureLoad(in_${l}, vec2u(inputPosition), 0)` : `in_${l}[inIdx]`;
    }
    const s = (
      /* wgsl */
      `
let x = i32(globalId.x);
let y = i32(globalId.y);
let inputPosition = clamp(
  vec2i(x, y) + inputOffset,
  vec2i(0),
  inputSize - vec2i(1)
);
let inIdx = inputPosition.y * inputSize.x + inputPosition.x;
let col = ${u("color")};

let outIdx = y * outputSize.x + x;

if (${t}) {
  // Denoise the inversed alpha. Or the anti aliased edge will be too dark after denoised
  out_color[outIdx] = vec3f(1.0 - col.a);
}
else if (${i}) {
  out_color[outIdx] = vec3f(HDRForward(col.r * inputScale), HDRForward(col.g * inputScale), HDRForward(col.b * inputScale));
}
else {
  out_color[outIdx] = col.rgb;
}
`
    );
    this._inputPassAux.setCSCode({
      csDefine: a,
      csMain: (
        /* wgsl */
        `
${s}
let alb = ${u("albedo")};
let nor = ${u("normal")};
out_normal[outIdx] = nor.rgb;
out_albedo[outIdx] = alb.rgb;
  `
      )
    }), this._inputPassColor.setCSCode({
      csDefine: a,
      csMain: (
        /* wgsl */
        `
${s}
`
      )
    }), this._outputPass.setCSCode({
      csDefine: (
        /* wgsl */
        `
${Lt}
const logXMax = ${mt};
${o}
`
      ),
      csMain: (
        /* wgsl */
        `
let x = i32(globalId.x);
let y = i32(globalId.y);
if (x >= outputSize.x || y >= outputSize.y) {
  return;
}
let inIdx = (y + inputOffset.y) * inputSize.x + x + inputOffset.x;
let outIdx = (y + outputOffset.y) * imageSize.x + x + outputOffset.x;
let col = in_color[inIdx];
let raw = ${e ? "textureLoad(in_raw, globalId.xy + vec2u(outputOffset), 0)" : "in_raw[outIdx]"};

if (${t}) {
  out_color[outIdx] = vec4f(raw.rgb, 1.0 - col.r);
}
else if (${i}) {
  out_color[outIdx] = vec4f(
    vec3f(HDRInverse(col.r), HDRInverse(col.g), HDRInverse(col.b)) / inputScale,
    // Pick the alpha
    raw.a
  );
}
else {
  out_color[outIdx] = vec4f(col.rgb, raw.a);
}
`
      )
    });
  }
  setImageSize(e, t) {
    this._inputPassAux.setUniform("inputSize", new Int32Array([e, t])), this._inputPassColor.setUniform("inputSize", new Int32Array([e, t])), this._outputPass.setUniform("imageSize", new Int32Array([e, t])), this._outputPass.setSize(e, t), this._copyPass.setSize(e, t), this._copyPass.setUniform("size", new Int32Array([e, t]));
  }
  setInputTile(e) {
    const t = new Int32Array([e.width, e.height]);
    [this._inputPassAux, this._inputPassColor].forEach((i) => {
      i.setUniform("inputOffset", new Int32Array([e.x, e.y])), i.setUniform("outputSize", t), i.setSize(t[0], t[1]);
    }), this._outputPass.setUniform("inputSize", t);
  }
  setOutputTile(e, t) {
    const i = this._outputPass, r = new Int32Array([e.width, e.height]), o = e.x - t.x, a = e.y - t.y;
    i.setUniform("outputSize", r), i.setUniform("inputOffset", new Int32Array([o, a])), i.setUniform(
      "outputOffset",
      new Int32Array([e.x, e.y])
    ), i.setExecuteSize(r[0], r[1]);
  }
  forward(e, t, i, r) {
    const o = e instanceof GPUTexture;
    this._updatePasses(o, r);
    const a = this._inputPassAux, u = this._inputPassColor, s = this._device.createCommandEncoder();
    function l(p) {
      return p instanceof GPUTexture ? {
        texture: p,
        channels: 4
      } : {
        buffer: p,
        channels: 4
      };
    }
    return t && i ? a.createPass(s, {
      color: l(e),
      albedo: l(t),
      normal: l(i)
    }) : u.createPass(s, {
      color: l(e)
    }), this._device.queue.submit([s.finish()]), t && i ? {
      color: a.getOutput("color"),
      albedo: a.getOutput("albedo"),
      normal: a.getOutput("normal")
    } : {
      color: u.getOutput("color")
    };
  }
  inverse(e, t) {
    const r = this._device.createCommandEncoder(), o = this._outputPass;
    return o.createPass(r, {
      color: { buffer: e, channels: 4 },
      raw: t instanceof GPUBuffer ? { buffer: t, channels: 4 } : { texture: t, channels: 4 }
    }), this._device.queue.submit([r.finish()]), o.getOutput("color");
  }
  copyInputDataToOutput(e) {
    const t = this._device.createCommandEncoder(), r = this._outputPass.getOutput("color"), o = this._copyPass;
    e instanceof GPUTexture ? (o.setOutputBuffers({
      color: r
    }), o.createPass(t, {
      color: { texture: e, channels: 4 }
    })) : t.copyBufferToBuffer(
      e,
      0,
      r,
      0,
      r.size
    ), this._device.queue.submit([t.finish()]);
  }
  dispose() {
    this._outputPass.dispose(), this._inputPassAux.dispose(), this._inputPassColor.dispose(), this._copyPass.dispose(!1);
  }
}
const Kn = 256, qn = 432, Zn = 16, Jn = 16, W = 16;
function we(n, e) {
  return Math.ceil(n / e) * e;
}
function Ni(n, e) {
  return Math.floor(n / e) * e;
}
function Wt(n, e, t) {
  const i = Math.round(t * n / e), r = Math.round((t + 1) * n / e);
  return { start: i, end: r };
}
function Le(n, e, t, i = 0) {
  const r = e - n, o = Math.max(
    i,
    we(r, W)
  );
  let a = n - Math.floor((o - r) / 2);
  return a = Xe(a, 0, Math.max(0, t - o)), { start: a, end: a + o };
}
function Dt(n, e, t, i) {
  if (!Number.isInteger(n) || n <= 0 || !Number.isInteger(e) || e <= 0)
    throw new Error("Tile grid dimensions must be positive integers");
  if (!Number.isFinite(t) || t <= 0)
    throw new Error("Maximum tile size must be positive");
  if (!Number.isFinite(i) || i < 0)
    throw new Error("Tile overlap must be non-negative");
  const r = Math.max(
    W,
    Ni(t, W)
  ), o = we(i, W), a = Math.max(1, Math.ceil(n / r)), u = Math.max(1, Math.ceil(e / r)), s = [];
  let l = 0, p = 0;
  for (let f = 0; f < u; f++) {
    const g = Wt(e, u, f);
    for (let y = 0; y < a; y++) {
      const v = Wt(n, a, y), _ = {
        start: y === 0 ? v.start : Math.max(0, v.start - o),
        end: y === a - 1 ? v.end : Math.min(n, v.end + o)
      }, w = {
        start: f === 0 ? g.start : Math.max(0, g.start - o),
        end: f === u - 1 ? g.end : Math.min(e, g.end + o)
      }, $ = Le(
        _.start,
        _.end,
        n
      ), x = Le(
        w.start,
        w.end,
        e
      ), B = {
        x: v.start,
        y: g.start,
        width: v.end - v.start,
        height: g.end - g.start
      }, k = {
        x: $.start,
        y: x.start,
        width: $.end - $.start,
        height: x.end - x.start
      };
      l = Math.max(l, B.width), p = Math.max(p, B.height), s.push({ column: y, row: f, input: k, output: B });
    }
  }
  const c = () => new Set(
    s.map(({ input: f }) => `${f.width}x${f.height}`)
  );
  if (c().size > 2) {
    const f = Math.max(
      ...s.map(({ input: _ }) => _.width)
    ), g = Math.max(
      ...s.map(({ input: _ }) => _.height)
    ), y = s.reduce(
      (_, { input: w }) => _ + f * w.height,
      0
    ), v = s.reduce(
      (_, { input: w }) => _ + w.width * g,
      0
    );
    if (y <= v)
      for (const _ of s) {
        const w = Le(
          _.input.x,
          _.input.x + _.input.width,
          n,
          f
        );
        _.input.x = w.start, _.input.width = w.end - w.start;
      }
    else
      for (const _ of s) {
        const w = Le(
          _.input.y,
          _.input.y + _.input.height,
          e,
          g
        );
        _.input.y = w.start, _.input.height = w.end - w.start;
      }
  }
  const d = c(), h = s.reduce(
    (f, { input: g }) => f + g.width * g.height,
    0
  );
  return {
    columns: a,
    rows: u,
    overlap: o,
    maxOutputWidth: l,
    maxOutputHeight: p,
    inputPixelCount: h,
    inputShapeCount: d.size,
    tiles: s
  };
}
function Xe(n, e, t) {
  return Math.min(Math.max(n, e), t);
}
function Qn(n, e) {
  const t = [...n].sort((i, r) => i - r);
  return t[Math.ceil(t.length * e) - 1];
}
class er {
  constructor(e, t = !0) {
    m(this, "enabled");
    m(this, "maxTileSize");
    m(this, "minTileSize");
    m(this, "targetTileTimeMs");
    m(this, "_tileSize");
    m(this, "_adjustmentStep");
    m(this, "_smoothedTileTimeMs");
    m(this, "_completeExecutionsSinceChange", 0);
    const i = typeof t == "object" ? t : {};
    this.enabled = t !== !1, this.maxTileSize = Math.max(
      W,
      Ni(e, W)
    ), this.minTileSize = Xe(
      we(i.minTileSize ?? Kn, W),
      W,
      this.maxTileSize
    ), this.targetTileTimeMs = Math.max(
      1,
      i.targetTileTimeMs ?? Zn
    ), this._adjustmentStep = Math.max(
      W,
      we(
        i.adjustmentStep ?? Jn,
        W
      )
    ), this._tileSize = this.enabled ? Xe(
      we(
        i.initialTileSize ?? qn,
        W
      ),
      this.minTileSize,
      this.maxTileSize
    ) : this.maxTileSize;
  }
  get tileSize() {
    return this._tileSize;
  }
  /** Returns true when the next execution should use a different tile size. */
  observe(e) {
    if (!this.enabled || e.length === 0) return !1;
    const t = e.filter(
      (a) => Number.isFinite(a) && a >= 0
    );
    if (t.length === 0) return !1;
    const i = t.length >= 3 ? t.slice(1) : t, r = Qn(i, 0.75);
    if (this._smoothedTileTimeMs = this._smoothedTileTimeMs === void 0 ? r : this._smoothedTileTimeMs * 0.65 + r * 0.35, this._completeExecutionsSinceChange++, this._completeExecutionsSinceChange < 2) return !1;
    let o = this._tileSize;
    return this._smoothedTileTimeMs > this.targetTileTimeMs * 1.25 ? o -= this._adjustmentStep : this._smoothedTileTimeMs < this.targetTileTimeMs * 0.65 && (o += this._adjustmentStep), o = Xe(
      we(o, W),
      this.minTileSize,
      this.maxTileSize
    ), o === this._tileSize ? !1 : (this._tileSize = o, this._smoothedTileTimeMs = void 0, this._completeExecutionsSinceChange = 0, !0);
  }
}
function I(n, e) {
  return {
    op: "conv2d",
    id: n,
    input: e,
    weight: `${n}.weight`,
    bias: `${n}.bias`,
    activation: "relu",
    padding: "same"
  };
}
function ue(n, e) {
  return {
    op: "maxPool2d",
    id: n,
    input: e,
    size: 2,
    stride: 2,
    padding: "same"
  };
}
function ce(n, e) {
  return {
    op: "upsample2d",
    id: n,
    input: e,
    scale: 2,
    mode: "nearest"
  };
}
function le(n, e, t) {
  return {
    op: "concat",
    id: n,
    inputs: [e, t],
    axis: "channels"
  };
}
const tr = {
  schemaVersion: 1,
  id: "oidn-unet-small-v1",
  family: "oidn-unet-small",
  input: "input",
  output: "dec_conv0",
  receptiveField: 174,
  nodes: [
    I("enc_conv0", "input"),
    I("enc_conv1", "enc_conv0"),
    ue("pool1", "enc_conv1"),
    I("enc_conv2", "pool1"),
    ue("pool2", "enc_conv2"),
    I("enc_conv3", "pool2"),
    ue("pool3", "enc_conv3"),
    I("enc_conv4", "pool3"),
    ue("pool4", "enc_conv4"),
    I("enc_conv5a", "pool4"),
    I("enc_conv5b", "enc_conv5a"),
    ce("up4", "enc_conv5b"),
    le("concat4", "up4", "pool3"),
    I("dec_conv4a", "concat4"),
    I("dec_conv4b", "dec_conv4a"),
    ce("up3", "dec_conv4b"),
    le("concat3", "up3", "pool2"),
    I("dec_conv3a", "concat3"),
    I("dec_conv3b", "dec_conv3a"),
    ce("up2", "dec_conv3b"),
    le("concat2", "up2", "pool1"),
    I("dec_conv2a", "concat2"),
    I("dec_conv2b", "dec_conv2a"),
    ce("up1", "dec_conv2b"),
    le("concat1", "up1", "input"),
    I("dec_conv1a", "concat1"),
    I("dec_conv1b", "dec_conv1a"),
    I("dec_conv0", "dec_conv1b")
  ]
}, ir = {
  schemaVersion: 1,
  id: "oidn-unet-large-v1",
  family: "oidn-unet-large",
  input: "input",
  output: "dec_conv1c",
  receptiveField: 202,
  nodes: [
    I("enc_conv1a", "input"),
    I("enc_conv1b", "enc_conv1a"),
    ue("pool1", "enc_conv1b"),
    I("enc_conv2a", "pool1"),
    I("enc_conv2b", "enc_conv2a"),
    ue("pool2", "enc_conv2b"),
    I("enc_conv3a", "pool2"),
    I("enc_conv3b", "enc_conv3a"),
    ue("pool3", "enc_conv3b"),
    I("enc_conv4a", "pool3"),
    I("enc_conv4b", "enc_conv4a"),
    ue("pool4", "enc_conv4b"),
    I("enc_conv5a", "pool4"),
    I("enc_conv5b", "enc_conv5a"),
    ce("up4", "enc_conv5b"),
    le("concat4", "up4", "pool3"),
    I("dec_conv4a", "concat4"),
    I("dec_conv4b", "dec_conv4a"),
    ce("up3", "dec_conv4b"),
    le("concat3", "up3", "pool2"),
    I("dec_conv3a", "concat3"),
    I("dec_conv3b", "dec_conv3a"),
    ce("up2", "dec_conv3b"),
    le("concat2", "up2", "pool1"),
    I("dec_conv2a", "concat2"),
    I("dec_conv2b", "dec_conv2a"),
    ce("up1", "dec_conv2b"),
    le("concat1", "up1", "input"),
    I("dec_conv1a", "concat1"),
    I("dec_conv1b", "dec_conv1a"),
    I("dec_conv1c", "dec_conv1b")
  ]
}, nr = [
  tr,
  ir
];
function Ui(n) {
  const e = /* @__PURE__ */ new Set();
  for (const t of n.nodes)
    t.op === "conv2d" && (e.add(t.weight), e.add(t.bias));
  return e;
}
function Rt(n) {
  return n.desc.getByteSize();
}
function zi(n) {
  return [...n].sort().join(", ");
}
function Li(n, e = nr) {
  const t = e.filter((i) => {
    const r = Ui(i);
    return [...r].some((o) => !n.has(o)) ? !1 : i.allowAdditionalTensors === !0 || [...n.keys()].every((o) => r.has(o));
  });
  if (t.length === 1) return t[0];
  throw t.length > 1 ? new Error(
    `Ambiguous OIDN model topology: ${t.map((i) => i.id).join(", ")}`
  ) : new Error(
    `Unsupported OIDN model topology. TZA tensors: ${zi(n.keys())}`
  );
}
function Yt(n, e, t) {
  const i = n.get(e);
  if (!i)
    throw new Error(`Model ${t} is missing tensor ${e}`);
  if (i.data.byteLength !== Rt(i))
    throw new Error(
      `Tensor ${e} has ${i.data.byteLength} bytes, expected ${Rt(i)}`
    );
  return i;
}
function rr(n, e = Li(n)) {
  if (e.schemaVersion !== 1)
    throw new Error(`Unsupported model descriptor schema ${e.schemaVersion}`);
  const t = Ui(e);
  if (!e.allowAdditionalTensors) {
    const c = [...n.keys()].filter((d) => !t.has(d));
    if (c.length > 0)
      throw new Error(
        `Model ${e.id} has unexpected tensors: ${zi(c)}`
      );
  }
  const i = /* @__PURE__ */ new Map(), r = /* @__PURE__ */ new Map(), o = /* @__PURE__ */ new Map(), a = /* @__PURE__ */ new Set([e.input]);
  let u, s;
  const l = (c, d) => {
    const h = i.get(c);
    if (h === void 0)
      throw new Error(
        `Model ${e.id} node ${d} reads unknown or forward value ${c}`
      );
    return h;
  };
  for (const c of e.nodes) {
    if (a.has(c.id))
      throw new Error(`Model ${e.id} produces duplicate value ${c.id}`);
    if (c.op === "conv2d") {
      const d = Yt(n, c.weight, e.id), h = Yt(n, c.bias, e.id), f = d.desc.dims;
      if (d.desc.layout !== "oihw" || f.length !== 4)
        throw new Error(`Tensor ${c.weight} must use OIHW layout`);
      if (f[2] !== 3 || f[3] !== 3)
        throw new Error(`Tensor ${c.weight} must use a 3x3 kernel`);
      if (h.desc.layout !== "x" || h.desc.dims.length !== 1)
        throw new Error(`Tensor ${c.bias} must be a one-dimensional bias`);
      if (h.desc.dims[0] !== f[0])
        throw new Error(
          `Tensor ${c.bias} has ${h.desc.dims[0]} channels, expected ${f[0]}`
        );
      if (d.desc.dataType !== h.desc.dataType)
        throw new Error(`Weight and bias dtype differ for ${c.id}`);
      if (s && s !== d.desc.dataType)
        throw new Error(`Mixed tensor dtypes are not supported by model ${e.id}`);
      s = d.desc.dataType, c.input === e.input && u === void 0 && (u = f[1], i.set(e.input, u));
      const g = l(c.input, c.id);
      if (g !== f[1])
        throw new Error(
          `Tensor ${c.weight} expects ${f[1]} input channels, but ${c.input} provides ${g}`
        );
      i.set(c.id, f[0]), r.set(c.id, {
        weight: d,
        bias: h,
        inputChannels: f[1],
        outputChannels: f[0],
        kernelHeight: f[2],
        kernelWidth: f[3]
      }), o.set(c.id, {
        inputChannels: f[1],
        outputChannels: f[0]
      });
    } else if (c.op === "concat") {
      if (c.inputs.length < 2)
        throw new Error(`Concat ${c.id} requires at least two inputs`);
      const d = c.inputs.reduce(
        (h, f) => h + l(f, c.id),
        0
      );
      i.set(c.id, d);
    } else
      i.set(c.id, l(c.input, c.id));
    a.add(c.id);
  }
  if (u === void 0 || s === void 0)
    throw new Error(`Model ${e.id} has no convolution reading its input`);
  const p = i.get(e.output);
  if (p === void 0)
    throw new Error(`Model ${e.id} output ${e.output} is not produced`);
  if (p !== 3)
    throw new Error(`Model ${e.id} must produce 3 channels, got ${p}`);
  return {
    spec: e,
    inputChannels: u,
    outputChannels: p,
    tensorDataType: s,
    channelsByValue: i,
    convChannels: o,
    convTensors: r
  };
}
const or = "This is not an object", ar = "This is not a Float16Array object", Gt = "This constructor is not a subclass of Float16Array", Wi = "The constructor property value is not an object", sr = "Species constructor didn't return TypedArray object", ur = "Derived constructor created TypedArray object which was too small length", Se = "Attempting to access detached ArrayBuffer", _t = "Cannot convert undefined or null to object", yt = "Cannot mix BigInt and other types, use explicit conversions", Xt = "@@iterator property is not callable", Ft = "Reduce of empty array with no initial value", cr = "The comparison function must be either a function or undefined", lt = "Offset is out of bounds";
function A(n) {
  return (e, ...t) => H(n, e, t);
}
function ke(n, e) {
  return A(
    xe(
      n,
      e
    ).get
  );
}
const {
  apply: H,
  construct: Ce,
  defineProperty: Vt,
  get: pt,
  getOwnPropertyDescriptor: xe,
  getPrototypeOf: Me,
  has: wt,
  ownKeys: Di,
  set: Ht,
  setPrototypeOf: Ri
} = Reflect, lr = Proxy, {
  EPSILON: pr,
  MAX_SAFE_INTEGER: jt,
  isFinite: Yi,
  isNaN: $e
} = Number, {
  iterator: ne,
  species: dr,
  toStringTag: Bt,
  for: hr
} = Symbol, be = Object, {
  create: Qe,
  defineProperty: Oe,
  freeze: fr,
  is: Kt
} = be, vt = be.prototype, gr = (
  /** @type {any} */
  vt.__lookupGetter__ ? A(
    /** @type {any} */
    vt.__lookupGetter__
  ) : (n, e) => {
    if (n == null)
      throw O(
        _t
      );
    let t = be(n);
    do {
      const i = xe(t, e);
      if (i !== void 0)
        return se(i, "get") ? i.get : void 0;
    } while ((t = Me(t)) !== null);
  }
), se = (
  /** @type {any} */
  be.hasOwn || A(vt.hasOwnProperty)
), Gi = Array, Xi = Gi.isArray, et = Gi.prototype, mr = A(et.join), _r = A(et.push), yr = A(
  et.toLocaleString
), Tt = et[ne], wr = A(Tt), {
  abs: vr,
  trunc: Fi
} = Math, tt = ArrayBuffer, xr = tt.isView, Vi = tt.prototype, $r = A(Vi.slice), br = ke(Vi, "byteLength"), xt = typeof SharedArrayBuffer < "u" ? SharedArrayBuffer : null, kr = xt && ke(xt.prototype, "byteLength"), Pt = Me(Uint8Array), Ir = Pt.from, G = Pt.prototype, Br = G[ne], Tr = A(G.keys), Pr = A(
  G.values
), Cr = A(
  G.entries
), Sr = A(G.set), qt = A(
  G.reverse
), Er = A(G.fill), Ar = A(
  G.copyWithin
), Zt = A(G.sort), Te = A(G.slice), Mr = A(
  G.subarray
), Y = ke(
  G,
  "buffer"
), he = ke(
  G,
  "byteOffset"
), P = ke(
  G,
  "length"
), Hi = ke(
  G,
  Bt
), Or = Uint8Array, K = Uint16Array, Jt = (...n) => H(Ir, K, n), Ct = Uint32Array, Nr = Float32Array, _e = Me([][ne]()), it = A(_e.next), Ur = A(function* () {
}().next), zr = Me(_e), O = TypeError, dt = RangeError, ji = WeakSet, Ki = ji.prototype, Lr = A(Ki.add), Wr = A(Ki.has), nt = WeakMap, St = nt.prototype, Ve = A(St.get), Dr = A(St.has), Et = A(St.set), qi = new nt(), Rr = Qe(null, {
  next: {
    value: function() {
      const e = Ve(qi, this);
      return it(e);
    }
  },
  [ne]: {
    value: function() {
      return this;
    }
  }
});
function We(n) {
  if (n[ne] === Tt && _e.next === it)
    return n;
  const e = Qe(Rr);
  return Et(qi, e, wr(n)), e;
}
const Zi = new nt(), Ji = Qe(zr, {
  next: {
    value: function() {
      const e = Ve(Zi, this);
      return Ur(e);
    },
    writable: !0,
    configurable: !0
  }
});
for (const n of Di(_e))
  n !== "next" && Oe(Ji, n, xe(_e, n));
function Qt(n) {
  const e = Qe(Ji);
  return Et(Zi, e, n), e;
}
function He(n) {
  return n !== null && typeof n == "object" || typeof n == "function";
}
function ei(n) {
  return n !== null && typeof n == "object";
}
function je(n) {
  return Hi(n) !== void 0;
}
function $t(n) {
  const e = Hi(n);
  return e === "BigInt64Array" || e === "BigUint64Array";
}
function Yr(n) {
  try {
    return Xi(n) ? !1 : (br(
      /** @type {any} */
      n
    ), !0);
  } catch {
    return !1;
  }
}
function Qi(n) {
  if (xt === null)
    return !1;
  try {
    return kr(
      /** @type {any} */
      n
    ), !0;
  } catch {
    return !1;
  }
}
function Gr(n) {
  return Yr(n) || Qi(n);
}
function ti(n) {
  return Xi(n) ? n[ne] === Tt && _e.next === it : !1;
}
function Xr(n) {
  return je(n) ? n[ne] === Br && _e.next === it : !1;
}
function De(n) {
  if (typeof n != "string")
    return !1;
  const e = +n;
  return n !== e + "" || !Yi(e) ? !1 : e === Fi(e);
}
const Ke = hr("__Float16Array__");
function Fr(n) {
  if (!ei(n))
    return !1;
  const e = Me(n);
  if (!ei(e))
    return !1;
  const t = e.constructor;
  if (t === void 0)
    return !1;
  if (!He(t))
    throw O(Wi);
  return wt(t, Ke);
}
const bt = 1 / pr;
function Vr(n) {
  return n + bt - bt;
}
const en = 6103515625e-14, Hr = 65504, tn = 9765625e-10, ii = tn * en, jr = tn * bt;
function Kr(n) {
  const e = +n;
  if (!Yi(e) || e === 0)
    return e;
  const t = e > 0 ? 1 : -1, i = vr(e);
  if (i < en)
    return t * Vr(i / ii) * ii;
  const r = (1 + jr) * i, o = r - (r - i);
  return o > Hr || $e(o) ? t * (1 / 0) : t * o;
}
const nn = new tt(4), rn = new Nr(nn), on = new Ct(nn), Z = new K(512), J = new Or(512);
for (let n = 0; n < 256; ++n) {
  const e = n - 127;
  e < -24 ? (Z[n] = 0, Z[n | 256] = 32768, J[n] = 24, J[n | 256] = 24) : e < -14 ? (Z[n] = 1024 >> -e - 14, Z[n | 256] = 1024 >> -e - 14 | 32768, J[n] = -e - 1, J[n | 256] = -e - 1) : e <= 15 ? (Z[n] = e + 15 << 10, Z[n | 256] = e + 15 << 10 | 32768, J[n] = 13, J[n | 256] = 13) : e < 128 ? (Z[n] = 31744, Z[n | 256] = 64512, J[n] = 24, J[n | 256] = 24) : (Z[n] = 31744, Z[n | 256] = 64512, J[n] = 13, J[n | 256] = 13);
}
function te(n) {
  rn[0] = Kr(n);
  const e = on[0], t = e >> 23 & 511;
  return Z[t] + ((e & 8388607) >> J[t]);
}
const At = new Ct(2048);
for (let n = 1; n < 1024; ++n) {
  let e = n << 13, t = 0;
  for (; !(e & 8388608); )
    e <<= 1, t -= 8388608;
  e &= -8388609, t += 947912704, At[n] = e | t;
}
for (let n = 1024; n < 2048; ++n)
  At[n] = 939524096 + (n - 1024 << 13);
const Ie = new Ct(64);
for (let n = 1; n < 31; ++n)
  Ie[n] = n << 23;
Ie[31] = 1199570944;
Ie[32] = 2147483648;
for (let n = 33; n < 63; ++n)
  Ie[n] = 2147483648 + (n - 32 << 23);
Ie[63] = 3347054592;
const an = new K(64);
for (let n = 1; n < 64; ++n)
  n !== 32 && (an[n] = 1024);
function S(n) {
  const e = n >> 10;
  return on[0] = At[an[e] + (n & 1023)] + Ie[e], rn[0];
}
function ae(n) {
  const e = +n;
  return $e(e) || e === 0 ? 0 : Fi(e);
}
function ht(n) {
  const e = ae(n);
  return e < 0 ? 0 : e < jt ? e : jt;
}
function Re(n, e) {
  if (!He(n))
    throw O(or);
  const t = n.constructor;
  if (t === void 0)
    return e;
  if (!He(t))
    throw O(Wi);
  const i = t[dr];
  return i ?? e;
}
function Ee(n) {
  if (Qi(n))
    return !1;
  try {
    return $r(n, 0, 0), !1;
  } catch {
  }
  return !0;
}
function ni(n, e) {
  const t = $e(n), i = $e(e);
  if (t && i)
    return 0;
  if (t)
    return 1;
  if (i || n < e)
    return -1;
  if (n > e)
    return 1;
  if (n === 0 && e === 0) {
    const r = Kt(n, 0), o = Kt(e, 0);
    if (!r && o)
      return -1;
    if (r && !o)
      return 1;
  }
  return 0;
}
const Mt = 2, qe = new nt();
function ve(n) {
  return Dr(qe, n) || !xr(n) && Fr(n);
}
function T(n) {
  if (!ve(n))
    throw O(ar);
}
function Ye(n, e) {
  const t = ve(n), i = je(n);
  if (!t && !i)
    throw O(sr);
  if (typeof e == "number") {
    let r;
    if (t) {
      const o = b(n);
      r = P(o);
    } else
      r = P(n);
    if (r < e)
      throw O(
        ur
      );
  }
  if ($t(n))
    throw O(yt);
}
function b(n) {
  const e = Ve(qe, n);
  if (e !== void 0) {
    const r = Y(e);
    if (Ee(r))
      throw O(Se);
    return e;
  }
  const t = (
    /** @type {any} */
    n.buffer
  );
  if (Ee(t))
    throw O(Se);
  const i = Ce(E, [
    t,
    /** @type {any} */
    n.byteOffset,
    /** @type {any} */
    n.length
  ], n.constructor);
  return Ve(qe, i);
}
function ri(n) {
  const e = P(n), t = [];
  for (let i = 0; i < e; ++i)
    t[i] = S(n[i]);
  return t;
}
const sn = new ji();
for (const n of Di(G)) {
  if (n === Bt)
    continue;
  const e = xe(G, n);
  se(e, "get") && typeof e.get == "function" && Lr(sn, e.get);
}
const qr = fr(
  /** @type {ProxyHandler<Float16BitsArray>} */
  {
    get(n, e, t) {
      return De(e) && se(n, e) ? S(pt(n, e)) : Wr(sn, gr(n, e)) ? pt(n, e) : pt(n, e, t);
    },
    set(n, e, t, i) {
      return De(e) && se(n, e) ? Ht(n, e, te(t)) : Ht(n, e, t, i);
    },
    getOwnPropertyDescriptor(n, e) {
      if (De(e) && se(n, e)) {
        const t = xe(n, e);
        return t.value = S(t.value), t;
      }
      return xe(n, e);
    },
    defineProperty(n, e, t) {
      return De(e) && se(n, e) && se(t, "value") && (t.value = te(t.value)), Vt(n, e, t);
    }
  }
);
class E {
  /** @see https://tc39.es/ecma262/#sec-typedarray */
  constructor(e, t, i) {
    let r;
    if (ve(e))
      r = Ce(K, [b(e)], new.target);
    else if (He(e) && !Gr(e)) {
      let a, u;
      if (je(e)) {
        a = e, u = P(e);
        const s = Y(e);
        if (Ee(s))
          throw O(Se);
        if ($t(e))
          throw O(yt);
        const l = new tt(
          u * Mt
        );
        r = Ce(K, [l], new.target);
      } else {
        const s = e[ne];
        if (s != null && typeof s != "function")
          throw O(Xt);
        s != null ? ti(e) ? (a = e, u = e.length) : (a = [.../** @type {Iterable<unknown>} */
        e], u = a.length) : (a = /** @type {ArrayLike<unknown>} */
        e, u = ht(a.length)), r = Ce(K, [u], new.target);
      }
      for (let s = 0; s < u; ++s)
        r[s] = te(a[s]);
    } else
      r = Ce(K, arguments, new.target);
    const o = (
      /** @type {any} */
      new lr(r, qr)
    );
    return Et(qe, o, r), o;
  }
  /**
   * limitation: `Object.getOwnPropertyNames(Float16Array)` or `Reflect.ownKeys(Float16Array)` include this key
   * @see https://tc39.es/ecma262/#sec-%typedarray%.from
   */
  static from(e, ...t) {
    const i = this;
    if (!wt(i, Ke))
      throw O(
        Gt
      );
    if (i === E) {
      if (ve(e) && t.length === 0) {
        const p = b(e), c = new K(
          Y(p),
          he(p),
          P(p)
        );
        return new E(
          Y(Te(c))
        );
      }
      if (t.length === 0)
        return new E(
          Y(
            Jt(e, te)
          )
        );
      const s = t[0], l = t[1];
      return new E(
        Y(
          Jt(e, function(p, ...c) {
            return te(
              H(s, this, [p, ...We(c)])
            );
          }, l)
        )
      );
    }
    let r, o;
    const a = e[ne];
    if (a != null && typeof a != "function")
      throw O(Xt);
    if (a != null)
      ti(e) ? (r = e, o = e.length) : Xr(e) ? (r = e, o = P(e)) : (r = [...e], o = r.length);
    else {
      if (e == null)
        throw O(
          _t
        );
      r = be(e), o = ht(r.length);
    }
    const u = new i(o);
    if (t.length === 0)
      for (let s = 0; s < o; ++s)
        u[s] = /** @type {number} */
        r[s];
    else {
      const s = t[0], l = t[1];
      for (let p = 0; p < o; ++p)
        u[p] = H(s, l, [r[p], p]);
    }
    return u;
  }
  /**
   * limitation: `Object.getOwnPropertyNames(Float16Array)` or `Reflect.ownKeys(Float16Array)` include this key
   * @see https://tc39.es/ecma262/#sec-%typedarray%.of
   */
  static of(...e) {
    const t = this;
    if (!wt(t, Ke))
      throw O(
        Gt
      );
    const i = e.length;
    if (t === E) {
      const o = new E(i), a = b(o);
      for (let u = 0; u < i; ++u)
        a[u] = te(e[u]);
      return o;
    }
    const r = new t(i);
    for (let o = 0; o < i; ++o)
      r[o] = e[o];
    return r;
  }
  /** @see https://tc39.es/ecma262/#sec-%typedarray%.prototype.keys */
  keys() {
    T(this);
    const e = b(this);
    return Tr(e);
  }
  /**
   * limitation: returns a object whose prototype is not `%ArrayIteratorPrototype%`
   * @see https://tc39.es/ecma262/#sec-%typedarray%.prototype.values
   */
  values() {
    T(this);
    const e = b(this);
    return Qt(function* () {
      for (const t of Pr(e))
        yield S(t);
    }());
  }
  /**
   * limitation: returns a object whose prototype is not `%ArrayIteratorPrototype%`
   * @see https://tc39.es/ecma262/#sec-%typedarray%.prototype.entries
   */
  entries() {
    T(this);
    const e = b(this);
    return Qt(function* () {
      for (const [t, i] of Cr(e))
        yield (
          /** @type {[number, number]} */
          [t, S(i)]
        );
    }());
  }
  /** @see https://tc39.es/ecma262/#sec-%typedarray%.prototype.at */
  at(e) {
    T(this);
    const t = b(this), i = P(t), r = ae(e), o = r >= 0 ? r : i + r;
    if (!(o < 0 || o >= i))
      return S(t[o]);
  }
  /** @see https://tc39.es/proposal-change-array-by-copy/#sec-%typedarray%.prototype.with */
  with(e, t) {
    T(this);
    const i = b(this), r = P(i), o = ae(e), a = o >= 0 ? o : r + o, u = +t;
    if (a < 0 || a >= r)
      throw dt(lt);
    const s = new K(
      Y(i),
      he(i),
      P(i)
    ), l = new E(
      Y(
        Te(s)
      )
    ), p = b(l);
    return p[a] = te(u), l;
  }
  /** @see https://tc39.es/ecma262/#sec-%typedarray%.prototype.map */
  map(e, ...t) {
    T(this);
    const i = b(this), r = P(i), o = t[0], a = Re(i, E);
    if (a === E) {
      const s = new E(r), l = b(s);
      for (let p = 0; p < r; ++p) {
        const c = S(i[p]);
        l[p] = te(
          H(e, o, [c, p, this])
        );
      }
      return s;
    }
    const u = new a(r);
    Ye(u, r);
    for (let s = 0; s < r; ++s) {
      const l = S(i[s]);
      u[s] = H(e, o, [l, s, this]);
    }
    return (
      /** @type {any} */
      u
    );
  }
  /** @see https://tc39.es/ecma262/#sec-%typedarray%.prototype.filter */
  filter(e, ...t) {
    T(this);
    const i = b(this), r = P(i), o = t[0], a = [];
    for (let l = 0; l < r; ++l) {
      const p = S(i[l]);
      H(e, o, [p, l, this]) && _r(a, p);
    }
    const u = Re(i, E), s = new u(a);
    return Ye(s), /** @type {any} */
    s;
  }
  /** @see https://tc39.es/ecma262/#sec-%typedarray%.prototype.reduce */
  reduce(e, ...t) {
    T(this);
    const i = b(this), r = P(i);
    if (r === 0 && t.length === 0)
      throw O(Ft);
    let o, a;
    t.length === 0 ? (o = S(i[0]), a = 1) : (o = t[0], a = 0);
    for (let u = a; u < r; ++u)
      o = e(
        o,
        S(i[u]),
        u,
        this
      );
    return o;
  }
  /** @see https://tc39.es/ecma262/#sec-%typedarray%.prototype.reduceright */
  reduceRight(e, ...t) {
    T(this);
    const i = b(this), r = P(i);
    if (r === 0 && t.length === 0)
      throw O(Ft);
    let o, a;
    t.length === 0 ? (o = S(i[r - 1]), a = r - 2) : (o = t[0], a = r - 1);
    for (let u = a; u >= 0; --u)
      o = e(
        o,
        S(i[u]),
        u,
        this
      );
    return o;
  }
  /** @see https://tc39.es/ecma262/#sec-%typedarray%.prototype.foreach */
  forEach(e, ...t) {
    T(this);
    const i = b(this), r = P(i), o = t[0];
    for (let a = 0; a < r; ++a)
      H(e, o, [
        S(i[a]),
        a,
        this
      ]);
  }
  /** @see https://tc39.es/ecma262/#sec-%typedarray%.prototype.find */
  find(e, ...t) {
    T(this);
    const i = b(this), r = P(i), o = t[0];
    for (let a = 0; a < r; ++a) {
      const u = S(i[a]);
      if (H(e, o, [u, a, this]))
        return u;
    }
  }
  /** @see https://tc39.es/ecma262/#sec-%typedarray%.prototype.findindex */
  findIndex(e, ...t) {
    T(this);
    const i = b(this), r = P(i), o = t[0];
    for (let a = 0; a < r; ++a) {
      const u = S(i[a]);
      if (H(e, o, [u, a, this]))
        return a;
    }
    return -1;
  }
  /** @see https://tc39.es/proposal-array-find-from-last/index.html#sec-%typedarray%.prototype.findlast */
  findLast(e, ...t) {
    T(this);
    const i = b(this), r = P(i), o = t[0];
    for (let a = r - 1; a >= 0; --a) {
      const u = S(i[a]);
      if (H(e, o, [u, a, this]))
        return u;
    }
  }
  /** @see https://tc39.es/proposal-array-find-from-last/index.html#sec-%typedarray%.prototype.findlastindex */
  findLastIndex(e, ...t) {
    T(this);
    const i = b(this), r = P(i), o = t[0];
    for (let a = r - 1; a >= 0; --a) {
      const u = S(i[a]);
      if (H(e, o, [u, a, this]))
        return a;
    }
    return -1;
  }
  /** @see https://tc39.es/ecma262/#sec-%typedarray%.prototype.every */
  every(e, ...t) {
    T(this);
    const i = b(this), r = P(i), o = t[0];
    for (let a = 0; a < r; ++a)
      if (!H(e, o, [
        S(i[a]),
        a,
        this
      ]))
        return !1;
    return !0;
  }
  /** @see https://tc39.es/ecma262/#sec-%typedarray%.prototype.some */
  some(e, ...t) {
    T(this);
    const i = b(this), r = P(i), o = t[0];
    for (let a = 0; a < r; ++a)
      if (H(e, o, [
        S(i[a]),
        a,
        this
      ]))
        return !0;
    return !1;
  }
  /** @see https://tc39.es/ecma262/#sec-%typedarray%.prototype.set */
  set(e, ...t) {
    T(this);
    const i = b(this), r = ae(t[0]);
    if (r < 0)
      throw dt(lt);
    if (e == null)
      throw O(
        _t
      );
    if ($t(e))
      throw O(
        yt
      );
    if (ve(e))
      return Sr(
        b(this),
        b(e),
        r
      );
    if (je(e)) {
      const s = Y(e);
      if (Ee(s))
        throw O(Se);
    }
    const o = P(i), a = be(e), u = ht(a.length);
    if (r === 1 / 0 || u + r > o)
      throw dt(lt);
    for (let s = 0; s < u; ++s)
      i[s + r] = te(a[s]);
  }
  /** @see https://tc39.es/ecma262/#sec-%typedarray%.prototype.reverse */
  reverse() {
    T(this);
    const e = b(this);
    return qt(e), this;
  }
  /** @see https://tc39.es/proposal-change-array-by-copy/#sec-%typedarray%.prototype.toReversed */
  toReversed() {
    T(this);
    const e = b(this), t = new K(
      Y(e),
      he(e),
      P(e)
    ), i = new E(
      Y(
        Te(t)
      )
    ), r = b(i);
    return qt(r), i;
  }
  /** @see https://tc39.es/ecma262/#sec-%typedarray%.prototype.fill */
  fill(e, ...t) {
    T(this);
    const i = b(this);
    return Er(
      i,
      te(e),
      ...We(t)
    ), this;
  }
  /** @see https://tc39.es/ecma262/#sec-%typedarray%.prototype.copywithin */
  copyWithin(e, t, ...i) {
    T(this);
    const r = b(this);
    return Ar(r, e, t, ...We(i)), this;
  }
  /** @see https://tc39.es/ecma262/#sec-%typedarray%.prototype.sort */
  sort(e) {
    T(this);
    const t = b(this), i = e !== void 0 ? e : ni;
    return Zt(t, (r, o) => i(S(r), S(o))), this;
  }
  /** @see https://tc39.es/proposal-change-array-by-copy/#sec-%typedarray%.prototype.toSorted */
  toSorted(e) {
    T(this);
    const t = b(this);
    if (e !== void 0 && typeof e != "function")
      throw new O(cr);
    const i = e !== void 0 ? e : ni, r = new K(
      Y(t),
      he(t),
      P(t)
    ), o = new E(
      Y(
        Te(r)
      )
    ), a = b(o);
    return Zt(a, (u, s) => i(S(u), S(s))), o;
  }
  /** @see https://tc39.es/ecma262/#sec-%typedarray%.prototype.slice */
  slice(e, t) {
    T(this);
    const i = b(this), r = Re(i, E);
    if (r === E) {
      const f = new K(
        Y(i),
        he(i),
        P(i)
      );
      return new E(
        Y(
          Te(f, e, t)
        )
      );
    }
    const o = P(i), a = ae(e), u = t === void 0 ? o : ae(t);
    let s;
    a === -1 / 0 ? s = 0 : a < 0 ? s = o + a > 0 ? o + a : 0 : s = o < a ? o : a;
    let l;
    u === -1 / 0 ? l = 0 : u < 0 ? l = o + u > 0 ? o + u : 0 : l = o < u ? o : u;
    const p = l - s > 0 ? l - s : 0, c = new r(p);
    if (Ye(c, p), p === 0)
      return c;
    const d = Y(i);
    if (Ee(d))
      throw O(Se);
    let h = 0;
    for (; s < l; )
      c[h] = S(i[s]), ++s, ++h;
    return (
      /** @type {any} */
      c
    );
  }
  /** @see https://tc39.es/ecma262/#sec-%typedarray%.prototype.subarray */
  subarray(e, t) {
    T(this);
    const i = b(this), r = Re(i, E), o = new K(
      Y(i),
      he(i),
      P(i)
    ), a = Mr(o, e, t), u = new r(
      Y(a),
      he(a),
      P(a)
    );
    return Ye(u), /** @type {any} */
    u;
  }
  /** @see https://tc39.es/ecma262/#sec-%typedarray%.prototype.indexof */
  indexOf(e, ...t) {
    T(this);
    const i = b(this), r = P(i);
    let o = ae(t[0]);
    if (o === 1 / 0)
      return -1;
    o < 0 && (o += r, o < 0 && (o = 0));
    for (let a = o; a < r; ++a)
      if (se(i, a) && S(i[a]) === e)
        return a;
    return -1;
  }
  /** @see https://tc39.es/ecma262/#sec-%typedarray%.prototype.lastindexof */
  lastIndexOf(e, ...t) {
    T(this);
    const i = b(this), r = P(i);
    let o = t.length >= 1 ? ae(t[0]) : r - 1;
    if (o === -1 / 0)
      return -1;
    o >= 0 ? o = o < r - 1 ? o : r - 1 : o += r;
    for (let a = o; a >= 0; --a)
      if (se(i, a) && S(i[a]) === e)
        return a;
    return -1;
  }
  /** @see https://tc39.es/ecma262/#sec-%typedarray%.prototype.includes */
  includes(e, ...t) {
    T(this);
    const i = b(this), r = P(i);
    let o = ae(t[0]);
    if (o === 1 / 0)
      return !1;
    o < 0 && (o += r, o < 0 && (o = 0));
    const a = $e(e);
    for (let u = o; u < r; ++u) {
      const s = S(i[u]);
      if (a && $e(s) || s === e)
        return !0;
    }
    return !1;
  }
  /** @see https://tc39.es/ecma262/#sec-%typedarray%.prototype.join */
  join(e) {
    T(this);
    const t = b(this), i = ri(t);
    return mr(i, e);
  }
  /** @see https://tc39.es/ecma262/#sec-%typedarray%.prototype.tolocalestring */
  toLocaleString(...e) {
    T(this);
    const t = b(this), i = ri(t);
    return yr(i, ...We(e));
  }
  /** @see https://tc39.es/ecma262/#sec-get-%typedarray%.prototype-@@tostringtag */
  get [Bt]() {
    if (ve(this))
      return (
        /** @type {any} */
        "Float16Array"
      );
  }
}
Oe(E, "BYTES_PER_ELEMENT", {
  value: Mt
});
Oe(E, Ke, {});
Ri(E, Pt);
const Ze = E.prototype;
Oe(Ze, "BYTES_PER_ELEMENT", {
  value: Mt
});
Oe(Ze, ne, {
  value: Ze.values,
  writable: !0,
  configurable: !0
});
Ri(Ze, G);
const ee = 8, ge = ee + 2, un = 3 * 3;
function oi(n, e, t = !1) {
  const i = n === "fp16" ? 2 : 4, r = ge * ge * e, o = t ? un * e * 4 : 0;
  return (r + o) * 4 * i;
}
function Zr(n) {
  return n === "fp16" ? "vec4<f16>" : "vec4<f32>";
}
function Jr(n) {
  return n === "fp16" ? `enable f16;
` : "";
}
function Qr(n, e, t) {
  const i = (r) => `${t}[weightBase + ${r}u]`;
  return n === "fp16" ? (
    /* wgsl */
    `
        let inputValue = vec4<f16>(${e});
        var partial = vec4<f16>(0.0h);
        partial = fma(${i(0)}, vec4<f16>(inputValue.x), partial);
        partial = fma(${i(1)}, vec4<f16>(inputValue.y), partial);
        partial = fma(${i(2)}, vec4<f16>(inputValue.z), partial);
        partial = fma(${i(3)}, vec4<f16>(inputValue.w), partial);
        acc += vec4<f32>(partial);
`
  ) : (
    /* wgsl */
    `
        let inputValue = vec4<f32>(${e});
        acc = fma(vec4<f32>(${i(0)}), vec4<f32>(inputValue.x), acc);
        acc = fma(vec4<f32>(${i(1)}), vec4<f32>(inputValue.y), acc);
        acc = fma(vec4<f32>(${i(2)}), vec4<f32>(inputValue.z), acc);
        acc = fma(vec4<f32>(${i(3)}), vec4<f32>(inputValue.w), acc);
`
  );
}
function eo(n, e, t, i = !1) {
  if (!Number.isInteger(t) || t < 1)
    throw new Error(`Final RGB shader requires positive input blocks, got ${t}`);
  const r = Zr(n), o = "vec4<f32>", a = ge * ge * t, u = un * t * 4, s = e === "relu" ? "max(acc, vec4<f32>(0.0))" : "acc", l = Qr(
    n,
    "inputTile[patchBase + inputBlock]",
    i ? "weightTile" : "weights"
  );
  return (
    /* wgsl */
    `${Jr(n)}
struct Params {
  inputWidth: u32,
  inputHeight: u32,
  outputWidth: u32,
  outputHeight: u32,
  inputBlocks: u32,
  outputBlocks: u32,
}

@group(0) @binding(0) var<storage, read> inputData: array<${r}>;
@group(0) @binding(1) var<storage, read> weights: array<${r}>;
@group(0) @binding(2) var<storage, read> bias: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> outputData: array<${o}>;
@group(0) @binding(4) var<uniform> params: Params;

var<workgroup> inputTile: array<${r}, ${a}>;
${i ? `var<workgroup> weightTile: array<${r}, ${u}>;` : ""}

@compute @workgroup_size(${ee}, ${ee}, 1)
fn main(
  @builtin(local_invocation_id) localId: vec3<u32>,
  @builtin(global_invocation_id) gid: vec3<u32>,
  @builtin(workgroup_id) workgroupId: vec3<u32>
) {
  let localLinear = localId.y * ${ee}u + localId.x;
  for (
    var loadIndex = localLinear;
    loadIndex < ${a}u;
    loadIndex += ${ee * ee}u
  ) {
    let tilePixel = loadIndex / ${t}u;
    let inputBlock = loadIndex % ${t}u;
    let tileX = tilePixel % ${ge}u;
    let tileY = tilePixel / ${ge}u;
    let inputX = i32(workgroupId.x * ${ee}u + tileX) - 1;
    let inputY = i32(workgroupId.y * ${ee}u + tileY) - 1;
    var value = ${r}(0.0);
    if (
      inputX >= 0 && inputX < i32(params.inputWidth) &&
      inputY >= 0 && inputY < i32(params.inputHeight)
    ) {
      let inputIndex =
        (u32(inputY) * params.inputWidth + u32(inputX)) *
        ${t}u + inputBlock;
      value = inputData[inputIndex];
    }
    inputTile[loadIndex] = value;
  }

${i ? `  for (
    var loadIndex = localLinear;
    loadIndex < ${u}u;
    loadIndex += ${ee * ee}u
  ) {
    weightTile[loadIndex] = weights[loadIndex];
  }

` : ""}  // Out-of-range invocations must reach this barrier before returning.
  workgroupBarrier();

  let outputInBounds =
    gid.x < params.outputWidth &&
    gid.y < params.outputHeight &&
    gid.z < params.outputBlocks;
  if (!outputInBounds) {
    return;
  }

  var acc = bias[gid.z];
  for (var ky = 0u; ky < 3u; ky++) {
    let inputY = i32(gid.y) + i32(ky) - 1;
    if (inputY < 0 || inputY >= i32(params.inputHeight)) {
      continue;
    }
    for (var kx = 0u; kx < 3u; kx++) {
      let inputX = i32(gid.x) + i32(kx) - 1;
      if (inputX < 0 || inputX >= i32(params.inputWidth)) {
        continue;
      }
      let patchBase =
        ((localId.y + ky) * ${ge}u + localId.x + kx) *
        ${t}u;
      for (var inputBlock = 0u; inputBlock < ${t}u; inputBlock++) {
        let weightBase =
          ((ky * 3u + kx) * ${t}u + inputBlock) * 4u;
        ${l}
      }
    }
  }

  let outputIndex =
    (gid.y * params.outputWidth + gid.x) * 1u + gid.z;
  outputData[outputIndex] = ${s};
}
`
  );
}
function to(n) {
  return n.op === "concat" ? n.inputs : [n.input];
}
function io(n) {
  const e = /* @__PURE__ */ new Map();
  for (const t of n.nodes)
    for (const i of to(t)) {
      const r = e.get(i) ?? [];
      r.push(t), e.set(i, r);
    }
  return e;
}
function ft(n, e, t) {
  const i = n.get(e);
  if (!((i == null ? void 0 : i.length) !== 1 || i[0].op !== t))
    return i[0];
}
function kt(n, e = {}) {
  const t = n.spec, i = io(t), r = new Map(t.nodes.map((p) => [p.id, p])), o = /* @__PURE__ */ new Set(), a = /* @__PURE__ */ new Map();
  let u = 0, s = 0;
  if (e.fuseConvPool !== !1)
    for (const p of t.nodes) {
      if (p.op !== "conv2d" || p.activation !== "relu") continue;
      const c = ft(i, p.id, "maxPool2d");
      !c || c.size !== 2 || c.stride !== 2 || (o.add(p.id), a.set(c.id, {
        op: "fusedConvReluMaxPool2d",
        id: c.id,
        input: p.input,
        conv: p,
        pool: c
      }), u++);
    }
  if (e.fuseUpsampleConcatConv !== !1)
    for (const p of t.nodes) {
      if (p.op !== "conv2d") continue;
      const c = r.get(p.input);
      if ((c == null ? void 0 : c.op) !== "concat" || c.inputs.length !== 2 || ft(i, c.id, "conv2d") !== p) continue;
      const d = n.channelsByValue.get(c.inputs[0]);
      if (d === void 0 || d % 4 !== 0)
        continue;
      const h = c.inputs.map((g) => {
        const y = r.get(g);
        return (y == null ? void 0 : y.op) === "upsample2d" && y.scale === 2 && y.mode === "nearest" && ft(i, y.id, "concat") === c ? { value: y.input, upsample: y } : { value: g };
      });
      if (h.filter((g) => g.upsample).length === 1) {
        o.add(c.id);
        for (const g of c.inputs) {
          const y = r.get(g);
          (y == null ? void 0 : y.op) === "upsample2d" && o.add(y.id);
        }
        a.set(p.id, {
          op: "fusedUpsampleConcatConv2d",
          id: p.id,
          inputs: h,
          conv: p
        }), s++;
      }
    }
  const l = [];
  for (const p of t.nodes) {
    const c = a.get(p.id);
    c ? l.push(c) : o.has(p.id) || l.push(p);
  }
  return {
    spec: t,
    nodes: l,
    fusions: { convPool: u, upsampleConcatConv: s }
  };
}
function no(n) {
  return n.op === "concat" ? n.inputs : n.op === "fusedUpsampleConcatConv2d" ? n.inputs.map((e) => e.value) : [n.input];
}
function ai(n, e) {
  return n.width === e.width && n.height === e.height;
}
function ro(n, e, t, i) {
  if (!Number.isInteger(e) || e <= 0 || !Number.isInteger(t) || t <= 0)
    throw new Error(`Invalid model input size ${e}x${t}`);
  const r = kt(n, i), o = { width: e, height: t, channels: n.inputChannels }, a = /* @__PURE__ */ new Map([
    [n.spec.input, o]
  ]), u = [], s = (c, d) => {
    const h = a.get(c);
    if (!h) throw new Error(`Planned node ${d} reads missing value ${c}`);
    return h;
  };
  for (const c of r.nodes) {
    let d;
    if (c.op === "conv2d") {
      const h = s(c.input, c.id);
      d = {
        width: h.width,
        height: h.height,
        channels: n.convChannels.get(c.id).outputChannels
      };
    } else if (c.op === "maxPool2d") {
      const h = s(c.input, c.id);
      d = {
        width: Math.ceil(h.width / 2),
        height: Math.ceil(h.height / 2),
        channels: h.channels
      };
    } else if (c.op === "upsample2d") {
      const h = s(c.input, c.id);
      d = {
        width: h.width * 2,
        height: h.height * 2,
        channels: h.channels
      };
    } else if (c.op === "concat") {
      const h = c.inputs.map((f) => s(f, c.id));
      if (h.some((f) => !ai(f, h[0])))
        throw new Error(`Concat ${c.id} has mismatched spatial shapes`);
      d = {
        width: h[0].width,
        height: h[0].height,
        channels: h.reduce((f, g) => f + g.channels, 0)
      };
    } else if (c.op === "fusedConvReluMaxPool2d") {
      const h = s(c.input, c.id);
      d = {
        width: Math.ceil(h.width / 2),
        height: Math.ceil(h.height / 2),
        channels: n.convChannels.get(c.conv.id).outputChannels
      };
    } else {
      const h = c.inputs.map((f) => {
        const g = s(f.value, c.id);
        return f.upsample ? { ...g, width: g.width * 2, height: g.height * 2 } : g;
      });
      if (h.some((f) => !ai(f, h[0])))
        throw new Error(`Fused decoder ${c.id} has mismatched spatial shapes`);
      d = {
        width: h[0].width,
        height: h[0].height,
        channels: n.convChannels.get(c.conv.id).outputChannels
      };
    }
    a.set(c.id, d), u.push(d);
  }
  const l = /* @__PURE__ */ new Map();
  r.nodes.forEach((c, d) => {
    for (const h of no(c)) l.set(h, d);
  }), l.set(n.spec.output, r.nodes.length);
  const p = r.nodes.map((c, d) => ({
    node: c,
    outputShape: u[d],
    lastUse: l.get(c.id) ?? d
  }));
  return { ...r, inputShape: o, valueShapes: a, plannedNodes: p };
}
class cn {
  constructor() {
    m(this, "_stats", /* @__PURE__ */ new Map());
  }
  track(e, t) {
    let i = this._stats.get(e);
    return i || (i = {
      created: 0,
      destroyed: 0,
      live: 0,
      peakLive: 0,
      resources: /* @__PURE__ */ new Set()
    }, this._stats.set(e, i)), i.resources.has(t) || (i.resources.add(t), i.created++, i.live++, i.peakLive = Math.max(i.peakLive, i.live)), t;
  }
  release(e, t, i) {
    if (!t) return !1;
    const r = this._stats.get(e);
    if (!(r != null && r.resources.delete(t))) return !1;
    try {
      i();
    } finally {
      r.destroyed++, r.live--;
    }
    return !0;
  }
  snapshot(e = 0) {
    let t = 0, i = 0, r = 0, o = 0;
    const a = {};
    for (const [u, s] of this._stats) {
      const l = {
        created: s.created,
        destroyed: s.destroyed,
        live: s.live,
        peakLive: s.peakLive
      };
      a[u] = l, t += l.live, i += l.created, r += l.destroyed, o += l.peakLive;
    }
    return { live: t, created: i, destroyed: r, peakLive: o, pending: e, byKind: a };
  }
}
const si = /* @__PURE__ */ new WeakMap();
function oo(n) {
  let e = si.get(n);
  return e || (e = { ready: /* @__PURE__ */ new Map(), pending: /* @__PURE__ */ new Map() }, si.set(n, e)), e;
}
const D = 8, ln = 8, U = 8, q = U + 2;
function ie(n) {
  const [e, t] = n.workgroupSize;
  return {
    workgroupX: e,
    workgroupY: t,
    rowsPerThread: n.rowsPerThread,
    tileM: t * n.rowsPerThread,
    tileNBlocks: e,
    // Keep the half partial accumulation grouping independent of tile tuning.
    tileKBlocks: ln
  };
}
function me(n) {
  const e = ie(n), t = n.sharedLayout === "padded" || n.sharedLayout === "padded-input", i = n.sharedLayout === "padded" || n.sharedLayout === "padded-weights";
  return {
    input: e.tileM * e.tileKBlocks + (t ? e.workgroupY : 0),
    weights: e.tileKBlocks * e.tileNBlocks * (i ? 5 : 4)
  };
}
function rt(n, e) {
  return e.sharedLayout === "padded" || e.sharedLayout === "padded-input" ? `(${n}) + (${n}) / ${e.rowsPerThread * ln}u` : n;
}
function pn(n, e) {
  return e.sharedLayout === "padded" || e.sharedLayout === "padded-weights" ? `(${n}) + (${n}) / 4u` : n;
}
function Ot(n, e) {
  return Math.ceil(n / e) * e;
}
function M(n) {
  return Math.ceil(n / 4);
}
function ao(n, e) {
  return n.width * n.height * M(n.channels) * 4 * e;
}
let Pe;
function dn(n) {
  const e = n & 32768 ? -1 : 1, t = n >>> 10 & 31, i = n & 1023;
  return t === 0 ? e * i * 2 ** -24 : t === 31 ? i === 0 ? e * (1 / 0) : NaN : e * (1 + i / 1024) * 2 ** (t - 15);
}
function so() {
  if (!Pe) {
    Pe = new Float32Array(65536);
    for (let n = 0; n < Pe.length; n++)
      Pe[n] = dn(n);
  }
  return Pe;
}
function ui(n) {
  if (n.desc.dataType === "Float32")
    return new Float32Array(
      n.data.buffer,
      n.data.byteOffset,
      n.data.byteLength / 4
    );
  const e = new Uint16Array(
    n.data.buffer,
    n.data.byteOffset,
    n.data.byteLength / 2
  ), t = new Float32Array(e.length);
  if (e.length < 4096)
    for (let i = 0; i < e.length; i++)
      t[i] = dn(e[i]);
  else {
    const i = so();
    for (let r = 0; r < e.length; r++)
      t[r] = i[e[r]];
  }
  return t;
}
function It(n, e, t, i) {
  const r = Ot(t.byteLength, 4), o = n.createBuffer({
    label: e,
    size: r,
    usage: i,
    mappedAtCreation: !0
  });
  return new Uint8Array(o.getMappedRange()).set(
    new Uint8Array(t.buffer, t.byteOffset, t.byteLength)
  ), o.unmap(), o;
}
function ci(n, e, t) {
  const i = new Uint32Array(Ot(t.length, 4));
  return i.set(t), It(n, e, i, GPUBufferUsage.UNIFORM);
}
function uo(n, e, t, i, r) {
  const o = M(t.inputChannels), a = M(t.outputChannels), u = a * t.kernelHeight * t.kernelWidth * o * 4 * 4, s = i === "fp16" && t.weight.desc.dataType === "Float16", l = s ? new Uint16Array(u) : i === "fp16" ? new E(u) : new Float32Array(u), p = s ? new Uint16Array(
    t.weight.data.buffer,
    t.weight.data.byteOffset,
    t.weight.data.byteLength / 2
  ) : ui(t.weight);
  for (let h = 0; h < a; h++)
    for (let f = 0; f < t.kernelHeight; f++)
      for (let g = 0; g < t.kernelWidth; g++)
        for (let y = 0; y < o; y++)
          for (let v = 0; v < 4; v++) {
            const _ = h * 4 + v;
            for (let w = 0; w < 4; w++) {
              const $ = y * 4 + w, B = (r === "k-major" ? (((f * t.kernelWidth + g) * o + y) * a + h) * 16 : (((h * t.kernelHeight + f) * t.kernelWidth + g) * o + y) * 16) + w * 4 + v;
              if (_ < t.outputChannels && $ < t.inputChannels) {
                const k = ((_ * t.inputChannels + $) * t.kernelHeight + f) * t.kernelWidth + g;
                l[B] = p[k];
              }
            }
          }
  const c = new Float32Array(a * 4);
  c.set(ui(t.bias));
  const d = It(
    n,
    `oidn/${e}/weights/${i}/${r}`,
    l,
    GPUBufferUsage.STORAGE
  );
  try {
    return {
      weights: d,
      weightLayout: r,
      bias: It(
        n,
        `oidn/${e}/bias`,
        c,
        GPUBufferUsage.STORAGE
      )
    };
  } catch (h) {
    throw d.destroy(), h;
  }
}
function L(n) {
  return n === "fp16" ? "vec4<f16>" : "vec4<f32>";
}
function re(n) {
  return n === "fp16" ? `enable f16;
` : "";
}
function oe(n, e) {
  return e === "fp16" ? `vec4<f16>(${n})` : n;
}
function de(n, e) {
  return e === "relu" ? `max(${n}, vec4<f32>(0.0))` : n;
}
function Ne(n, e, t) {
  return t === "fp32" ? (
    /* wgsl */
    `
let inputValue = vec4<f32>(${n});
let weightBase = ${e};
acc = fma(vec4<f32>(weights[weightBase]), vec4<f32>(inputValue.x), acc);
acc = fma(vec4<f32>(weights[weightBase + 1u]), vec4<f32>(inputValue.y), acc);
acc = fma(vec4<f32>(weights[weightBase + 2u]), vec4<f32>(inputValue.z), acc);
acc = fma(vec4<f32>(weights[weightBase + 3u]), vec4<f32>(inputValue.w), acc);
`
  ) : (
    /* wgsl */
    `
let inputValue = vec4<f16>(${n});
let weightBase = ${e};
var partial = vec4<f16>(0.0h);
partial = fma(weights[weightBase], vec4<f16>(inputValue.x), partial);
partial = fma(weights[weightBase + 1u], vec4<f16>(inputValue.y), partial);
partial = fma(weights[weightBase + 2u], vec4<f16>(inputValue.z), partial);
partial = fma(weights[weightBase + 3u], vec4<f16>(inputValue.w), partial);
acc += vec4<f32>(partial);
`
  );
}
function co(n, e, t) {
  const i = t === "fp16" ? "vec4<f16>" : "vec4<f32>", r = t === "fp16" ? `var partial = vec4<f16>(0.0h);
partial = fma(subgroupBroadcast(weights[weightBase], 0u), ${i}(inputValue.x), partial);
partial = fma(subgroupBroadcast(weights[weightBase + 1u], 0u), ${i}(inputValue.y), partial);
partial = fma(subgroupBroadcast(weights[weightBase + 2u], 0u), ${i}(inputValue.z), partial);
partial = fma(subgroupBroadcast(weights[weightBase + 3u], 0u), ${i}(inputValue.w), partial);
acc += vec4<f32>(partial);` : `acc = fma(subgroupBroadcast(weights[weightBase], 0u), vec4<f32>(inputValue.x), acc);
acc = fma(subgroupBroadcast(weights[weightBase + 1u], 0u), vec4<f32>(inputValue.y), acc);
acc = fma(subgroupBroadcast(weights[weightBase + 2u], 0u), vec4<f32>(inputValue.z), acc);
acc = fma(subgroupBroadcast(weights[weightBase + 3u], 0u), vec4<f32>(inputValue.w), acc);`;
  return (
    /* wgsl */
    `
let inputValue = ${i}(${n});
let weightBase = ${e};
${r}
`
  );
}
function Je(n, e, t) {
  return n === "fp16" && (e.loadMode === "packed-all" || t && e.loadMode === "packed-weights");
}
function Ae(n, e) {
  return e ? `bitcast<vec4<f16>>(${n})` : n;
}
function hn(n, e) {
  const { rowsPerThread: t, tileNBlocks: i, tileKBlocks: r } = ie(e), o = rt(`tileSpatial * ${r}u + tileK`, e), a = `(tileK * ${i}u + localId.x) * ${e.sharedLayout === "padded" || e.sharedLayout === "padded-weights" ? 5 : 4}u`;
  if (e.accumulationOrder === "row-major") {
    const u = L(n), s = n === "fp16" ? "partial" : "acc[row]";
    return (
      /* wgsl */
      `
    for (var row = 0u; row < ${t}u; row++) {
      ${n === "fp16" ? "var partial = vec4<f16>(0.0h);" : ""}
      let tileSpatial = localId.y * ${t}u + row;
      for (var tileK = 0u; tileK < ${r}u; tileK++) {
        let weightBase = ${a};
        let inputValue = inputTile[${o}];
        ${s} = fma(weightTile[weightBase], ${u}(inputValue.x), ${s});
        ${s} = fma(weightTile[weightBase + 1u], ${u}(inputValue.y), ${s});
        ${s} = fma(weightTile[weightBase + 2u], ${u}(inputValue.z), ${s});
        ${s} = fma(weightTile[weightBase + 3u], ${u}(inputValue.w), ${s});
      }
      ${n === "fp16" ? "acc[row] += vec4<f32>(partial);" : ""}
    }
`
    );
  }
  return n === "fp16" ? (
    /* wgsl */
    `
    var partial: array<vec4<f16>, ${t}>;
    for (var row = 0u; row < ${t}u; row++) {
      partial[row] = vec4<f16>(0.0h);
    }
    for (var tileK = 0u; tileK < ${r}u; tileK++) {
      let weightBase =
        ${a};
      for (var row = 0u; row < ${t}u; row++) {
        let tileSpatial =
          localId.y * ${t}u + row;
        let inputValue =
          inputTile[${o}];
        partial[row] = fma(
          weightTile[weightBase],
          vec4<f16>(inputValue.x),
          partial[row]
        );
        partial[row] = fma(
          weightTile[weightBase + 1u],
          vec4<f16>(inputValue.y),
          partial[row]
        );
        partial[row] = fma(
          weightTile[weightBase + 2u],
          vec4<f16>(inputValue.z),
          partial[row]
        );
        partial[row] = fma(
          weightTile[weightBase + 3u],
          vec4<f16>(inputValue.w),
          partial[row]
        );
      }
    }
    for (var row = 0u; row < ${t}u; row++) {
      acc[row] += vec4<f32>(partial[row]);
    }
`
  ) : (
    /* wgsl */
    `
    for (var tileK = 0u; tileK < ${r}u; tileK++) {
      let weightBase =
        ${a};
      for (var row = 0u; row < ${t}u; row++) {
        let tileSpatial =
          localId.y * ${t}u + row;
        let inputValue =
          inputTile[${o}];
        acc[row] = fma(
          weightTile[weightBase],
          vec4<f32>(inputValue.x),
          acc[row]
        );
        acc[row] = fma(
          weightTile[weightBase + 1u],
          vec4<f32>(inputValue.y),
          acc[row]
        );
        acc[row] = fma(
          weightTile[weightBase + 2u],
          vec4<f32>(inputValue.z),
          acc[row]
        );
        acc[row] = fma(
          weightTile[weightBase + 3u],
          vec4<f32>(inputValue.w),
          acc[row]
        );
      }
    }
`
  );
}
function lo(n, e, t, i, r) {
  const o = L(n), a = L(n), u = L(e), s = oe(
    de("acc", t),
    e
  );
  return (
    /* wgsl */
    `${re(n)}
struct Params {
  inputWidth: u32,
  inputHeight: u32,
  outputWidth: u32,
  outputHeight: u32,
  inputBlocks: u32,
  outputBlocks: u32,
}

@group(0) @binding(0) var<storage, read> inputData: array<${o}>;
@group(0) @binding(1) var<storage, read> weights: array<${a}>;
@group(0) @binding(2) var<storage, read> bias: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> outputData: array<${u}>;
@group(0) @binding(4) var<uniform> params: Params;

@compute @workgroup_size(${D}, ${D}, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= params.outputWidth || gid.y >= params.outputHeight || gid.z >= ${r}u) {
    return;
  }
  var acc = bias[gid.z];
  for (var ky = 0u; ky < 3u; ky++) {
    let inputY = i32(gid.y) + i32(ky) - 1;
    if (inputY < 0 || inputY >= i32(params.inputHeight)) { continue; }
    for (var kx = 0u; kx < 3u; kx++) {
      let inputX = i32(gid.x) + i32(kx) - 1;
      if (inputX < 0 || inputX >= i32(params.inputWidth)) { continue; }
      let pixelBase = (u32(inputY) * params.inputWidth + u32(inputX)) * ${i}u;
      for (var inputBlock = 0u; inputBlock < ${i}u; inputBlock++) {
        ${Ne(
      "inputData[pixelBase + inputBlock]",
      `((((gid.z * 3u + ky) * 3u + kx) * ${i}u + inputBlock) * 4u)`,
      n
    )}
      }
    }
  }
  let outputIndex = (gid.y * params.outputWidth + gid.x) * ${r}u + gid.z;
  outputData[outputIndex] = ${s};
}
`
  );
}
function po(n, e, t, i, r) {
  const o = L(n), a = L(n), u = L(e), s = oe(
    de("acc", t),
    e
  );
  return (
    /* wgsl */
    `${re(n)}
enable subgroups;
struct Params {
  inputWidth: u32,
  inputHeight: u32,
  outputWidth: u32,
  outputHeight: u32,
  inputBlocks: u32,
  outputBlocks: u32,
}
@group(0) @binding(0) var<storage, read> inputData: array<${o}>;
@group(0) @binding(1) var<storage, read> weights: array<${a}>;
@group(0) @binding(2) var<storage, read> bias: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> outputData: array<${u}>;
@group(0) @binding(4) var<uniform> params: Params;

@compute @workgroup_size(${D}, ${D}, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let outputInBounds =
    gid.x < params.outputWidth && gid.y < params.outputHeight;
  var acc = bias[gid.z];
  for (var ky = 0u; ky < 3u; ky++) {
    let inputY = i32(gid.y) + i32(ky) - 1;
    let clampedY = u32(clamp(inputY, 0, i32(params.inputHeight) - 1));
    for (var kx = 0u; kx < 3u; kx++) {
      let inputX = i32(gid.x) + i32(kx) - 1;
      let clampedX = u32(clamp(inputX, 0, i32(params.inputWidth) - 1));
      let inputInBounds =
        outputInBounds && inputX >= 0 && inputY >= 0 &&
        inputX < i32(params.inputWidth) && inputY < i32(params.inputHeight);
      let pixelBase =
        (clampedY * params.inputWidth + clampedX) * ${i}u;
      for (var inputBlock = 0u; inputBlock < ${i}u; inputBlock++) {
        ${co(
      `select(${o}(0.0), inputData[pixelBase + inputBlock], inputInBounds)`,
      `((((gid.z * 3u + ky) * 3u + kx) * ${i}u + inputBlock) * 4u)`,
      n
    )}
      }
    }
  }
  if (outputInBounds) {
    let outputIndex =
      (gid.y * params.outputWidth + gid.x) * ${r}u + gid.z;
    outputData[outputIndex] = ${s};
  }
}
`
  );
}
function ho(n, e, t, i, r) {
  const o = L(n), a = L(e), u = oe(
    de("acc", t),
    e
  ), s = q * q * i, l = U * U;
  return (
    /* wgsl */
    `${re(n)}
struct Params {
  inputWidth: u32,
  inputHeight: u32,
  outputWidth: u32,
  outputHeight: u32,
  inputBlocks: u32,
  outputBlocks: u32,
}
@group(0) @binding(0) var<storage, read> inputData: array<${o}>;
@group(0) @binding(1) var<storage, read> weights: array<${o}>;
@group(0) @binding(2) var<storage, read> bias: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> outputData: array<${a}>;
@group(0) @binding(4) var<uniform> params: Params;

var<workgroup> inputPatch: array<${o}, ${s}>;

@compute @workgroup_size(${U}, ${U}, 1)
fn main(
  @builtin(local_invocation_id) localId: vec3<u32>,
  @builtin(workgroup_id) workgroupId: vec3<u32>
) {
  let localLinear =
    localId.y * ${U}u + localId.x;
  for (
    var loadIndex = localLinear;
    loadIndex < ${s}u;
    loadIndex += ${l}u
  ) {
    let patchPixel = loadIndex / ${i}u;
    let inputBlock = loadIndex % ${i}u;
    let patchX = patchPixel % ${q}u;
    let patchY = patchPixel / ${q}u;
    let inputX =
      i32(workgroupId.x * ${U}u + patchX) - 1;
    let inputY =
      i32(workgroupId.y * ${U}u + patchY) - 1;
    var value = ${o}(0.0);
    if (
      inputX >= 0 && inputX < i32(params.inputWidth) &&
      inputY >= 0 && inputY < i32(params.inputHeight)
    ) {
      let inputIndex =
        (u32(inputY) * params.inputWidth + u32(inputX)) *
        ${i}u + inputBlock;
      value = inputData[inputIndex];
    }
    inputPatch[loadIndex] = value;
  }
  workgroupBarrier();

  let outputX =
    workgroupId.x * ${U}u + localId.x;
  let outputY =
    workgroupId.y * ${U}u + localId.y;
  let outputBlock = workgroupId.z;
  if (
    outputX >= params.outputWidth || outputY >= params.outputHeight ||
    outputBlock >= ${r}u
  ) {
    return;
  }

  var acc = bias[outputBlock];
  for (var ky = 0u; ky < 3u; ky++) {
    for (var kx = 0u; kx < 3u; kx++) {
      let patchBase =
        ((localId.y + ky) * ${q}u + localId.x + kx) *
        ${i}u;
      for (var inputBlock = 0u; inputBlock < ${i}u; inputBlock++) {
        ${Ne(
      "inputPatch[patchBase + inputBlock]",
      `((((outputBlock * 3u + ky) * 3u + kx) * ${i}u + inputBlock) * 4u)`,
      n
    )}
      }
    }
  }
  let outputIndex =
    (outputY * params.outputWidth + outputX) * ${r}u + outputBlock;
  outputData[outputIndex] = ${u};
}
`
  );
}
function fn(n, e, t, i) {
  const { workgroupX: r, workgroupY: o, tileM: a, tileKBlocks: u } = ie(t), s = e ? "params.outputWidth" : "params.inputWidth", l = e ? "params.outputHeight" : "params.inputHeight", p = t.addressMode === "base-offset", c = i ? i.blocks.map(
    (h, f) => `cachedBase${f}[load] = (${f === i.upsampled ? "y / 2u" : "y"} * params.source${f}Width + ${f === i.upsampled ? "x / 2u" : "x"}) * ${h}u;`
  ).join(`
    `) : `cachedBase0[load] = (y * params.inputWidth + x) * ${n}u;`, d = a * u / (r * o);
  return (
    /* wgsl */
    `
  ${p ? `var cachedBase0: array<u32, ${d}>;
  ${i ? `var cachedBase1: array<u32, ${d}>;` : ""}` : `var cachedX: array<u32, ${d}>;
  var cachedY: array<u32, ${d}>;`}
  var cachedMask: array<u32, ${d}>;
  for (var load = 0u; load < ${d}u; load++) {
    let loadIndex = localLinear + load * ${r * o}u;
    let spatial = workgroupId.x * ${a}u + loadIndex / ${u}u;
    let x = spatial % params.outputWidth;
    let y = spatial / params.outputWidth;
    ${p ? c : `cachedX[load] = x;
    cachedY[load] = y;`}
    let columns =
      select(0u, 0x049u, x > 0u && x - 1u < ${s}) |
      select(0u, 0x092u, x < ${s}) |
      select(0u, 0x124u, x + 1u < ${s});
    let rows =
      select(0u, 0x007u, y > 0u && y - 1u < ${l}) |
      select(0u, 0x038u, y < ${l}) |
      select(0u, 0x1c0u, y + 1u < ${l});
    cachedMask[load] = select(0u, columns & rows, spatial < spatialCount)${p && i ? " | ((x & 1u) << 9u) | ((y & 1u) << 10u)" : ""};
  }
  var channelBlock = (localLinear % ${u}u) % ${n}u;
  var filterPosition = (localLinear % ${u}u) / ${n}u;
`
  );
}
function gn(n, e) {
  const { tileKBlocks: t } = ie(e);
  return (
    /* wgsl */
    `
    channelBlock += ${t % n}u;
    let carry = channelBlock >= ${n}u;
    channelBlock -= select(0u, ${n}u, carry);
    filterPosition += ${Math.floor(t / n)}u + select(0u, 1u, carry);
`
  );
}
function Fe(n, e, t) {
  const { workgroupX: i, workgroupY: r, tileM: o, tileKBlocks: a } = ie(t);
  return (
    /* wgsl */
    `
    let inputBlock = channelBlock;
    let kernelY = filterPosition / 3u;
    let kernelX = filterPosition % 3u;
    for (var load = 0u; load < ${o * a / (i * r)}u; load++) {
      let loadIndex = localLinear + load * ${i * r}u;
      var value = ${n}(0.0);
      if (filterPosition < 9u && (cachedMask[load] & (1u << filterPosition)) != 0u) {
        ${t.addressMode === "base-offset" ? "" : `let inputX = i32(cachedX[load]) + i32(kernelX) - 1;
        let inputY = i32(cachedY[load]) + i32(kernelY) - 1;`}
        ${e}
      }
      inputTile[${rt("loadIndex", t)}] = value;
    }
`
  );
}
function fo(n, e, t, i, r, o) {
  const { workgroupX: a, workgroupY: u, rowsPerThread: s, tileM: l, tileNBlocks: p, tileKBlocks: c } = ie(o), { addressMode: d, weightLayout: h } = o, f = L(n), g = L(e), y = Je(n, o, !1), v = Je(n, o, !0), _ = y ? "vec2<u32>" : f, w = v ? "vec2<u32>" : f, $ = oe(
    de("acc", t),
    e
  ), x = a * u, B = l * c, k = c * p * 4;
  return (
    /* wgsl */
    `${re(n)}
struct Params {
  inputWidth: u32,
  inputHeight: u32,
  outputWidth: u32,
  outputHeight: u32,
  inputBlocks: u32,
  outputBlocks: u32,
}
@group(0) @binding(0) var<storage, read> inputData: array<${_}>;
@group(0) @binding(1) var<storage, read> weights: array<${w}>;
@group(0) @binding(2) var<storage, read> bias: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> outputData: array<${g}>;
@group(0) @binding(4) var<uniform> params: Params;

var<workgroup> inputTile: array<${f}, ${me(o).input}>;
var<workgroup> weightTile: array<${f}, ${me(o).weights}>;

@compute @workgroup_size(${a}, ${u}, 1)
fn main(
  @builtin(local_invocation_id) localId: vec3<u32>,
  @builtin(workgroup_id) workgroupId: vec3<u32>
) {
  let spatialBase =
    workgroupId.x * ${l}u +
    localId.y * ${s}u;
  let outputBlock =
    workgroupId.y * ${p}u + localId.x;
  let spatialCount = params.outputWidth * params.outputHeight;
  var acc: array<vec4<f32>, ${s}>;
  if (outputBlock < ${r}u) {
    for (var row = 0u; row < ${s}u; row++) {
      acc[row] = bias[outputBlock];
    }
  }

  let localLinear =
    localId.y * ${a}u + localId.x;
  ${d !== "analytic" ? fn(i, !1, o) : ""}
  let totalK = ${i * 9}u;
  for (var kBase = 0u; kBase < totalK; kBase += ${c}u) {
    ${d !== "analytic" ? Fe(f, `
        ${d === "base-offset" ? `let offset = (i32(kernelY) - 1) * i32(params.inputWidth) + i32(kernelX) - 1;
        let inputIndex = cachedBase0[load] + u32(offset * ${i}) + inputBlock;` : `let inputIndex = (u32(inputY) * params.inputWidth + u32(inputX)) *
          ${i}u + inputBlock;`}
        value = ${Ae("inputData[inputIndex]", y)};
    `, o) : (
      /* wgsl */
      `
    for (
      var loadIndex = localLinear;
      loadIndex < ${B}u;
      loadIndex += ${x}u
    ) {
      let tileSpatial = loadIndex / ${c}u;
      let tileK = loadIndex % ${c}u;
      let inputSpatialIndex =
        workgroupId.x * ${l}u + tileSpatial;
      let kIndex = kBase + tileK;
      var value = ${f}(0.0);
      if (inputSpatialIndex < spatialCount && kIndex < totalK) {
        let outputY = inputSpatialIndex / params.outputWidth;
        let outputX = inputSpatialIndex % params.outputWidth;
        let inputBlock = kIndex % ${i}u;
        let kernelIndex = kIndex / ${i}u;
        let kernelY = kernelIndex / 3u;
        let kernelX = kernelIndex % 3u;
        let inputY = i32(outputY) + i32(kernelY) - 1;
        let inputX = i32(outputX) + i32(kernelX) - 1;
        if (
          inputY >= 0 && inputY < i32(params.inputHeight) &&
          inputX >= 0 && inputX < i32(params.inputWidth)
        ) {
          let inputIndex =
            (u32(inputY) * params.inputWidth + u32(inputX)) *
            ${i}u + inputBlock;
          value = ${Ae("inputData[inputIndex]", y)};
        }
      }
      inputTile[${rt("loadIndex", o)}] = value;
    }
    `
    )}

    for (
      var loadIndex = localLinear;
      loadIndex < ${k}u;
      loadIndex += ${x}u
    ) {
      let tileK = loadIndex / ${p * 4}u;
      let outputRemainder = loadIndex % ${p * 4}u;
      let tileOutputBlock = outputRemainder / 4u;
      let outputLane = outputRemainder % 4u;
      let loadedOutputBlock =
        workgroupId.y * ${p}u + tileOutputBlock;
      let kIndex = kBase + tileK;
      var value = ${f}(0.0);
      if (loadedOutputBlock < ${r}u && kIndex < totalK) {
        ${h === "k-major" ? (
      /* wgsl */
      `
        let weightIndex = (kIndex * ${r}u + loadedOutputBlock) * 4u + outputLane;
        `
    ) : d !== "analytic" ? (
      /* wgsl */
      `
        let weightIndex = (loadedOutputBlock * ${i * 9}u + kIndex) * 4u + outputLane;
        `
    ) : (
      /* wgsl */
      `
        let inputBlock = kIndex % ${i}u;
        let kernelIndex = kIndex / ${i}u;
        let kernelY = kernelIndex / 3u;
        let kernelX = kernelIndex % 3u;
        let weightIndex =
          ((((loadedOutputBlock * 3u + kernelY) * 3u + kernelX) *
            ${i}u + inputBlock) * 4u + outputLane);
        `
    )}
        value = ${Ae("weights[weightIndex]", v)};
      }
      weightTile[${pn("loadIndex", o)}] = value;
    }

    workgroupBarrier();
    ${hn(n, o)}
    workgroupBarrier();
    ${d !== "analytic" ? gn(i, o) : ""}
  }

  if (outputBlock < ${r}u) {
    for (var row = 0u; row < ${s}u; row++) {
      let spatialIndex = spatialBase + row;
      if (spatialIndex < spatialCount) {
        let outputIndex = spatialIndex * ${r}u + outputBlock;
        outputData[outputIndex] = ${$.replaceAll("acc", "acc[row]")};
      }
    }
  }
}
`
  );
}
function go(n, e, t, i) {
  const r = L(n), o = de("acc", e);
  return (
    /* wgsl */
    `${re(n)}
struct Params {
  inputWidth: u32,
  inputHeight: u32,
  outputWidth: u32,
  outputHeight: u32,
  inputBlocks: u32,
  outputBlocks: u32,
}
@group(0) @binding(0) var<storage, read> inputData: array<${r}>;
@group(0) @binding(1) var<storage, read> weights: array<${r}>;
@group(0) @binding(2) var<storage, read> bias: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> outputData: array<${r}>;
@group(0) @binding(4) var<uniform> params: Params;

@compute @workgroup_size(${D}, ${D}, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= params.outputWidth || gid.y >= params.outputHeight || gid.z >= ${i}u) {
    return;
  }
  var pooled = vec4<f32>(-3.402823466e+38);
  for (var py = 0u; py < 2u; py++) {
    let centerY = gid.y * 2u + py;
    if (centerY >= params.inputHeight) { continue; }
    for (var px = 0u; px < 2u; px++) {
      let centerX = gid.x * 2u + px;
      if (centerX >= params.inputWidth) { continue; }
      var acc = bias[gid.z];
      for (var ky = 0u; ky < 3u; ky++) {
        let inputY = i32(centerY) + i32(ky) - 1;
        if (inputY < 0 || inputY >= i32(params.inputHeight)) { continue; }
        for (var kx = 0u; kx < 3u; kx++) {
          let inputX = i32(centerX) + i32(kx) - 1;
          if (inputX < 0 || inputX >= i32(params.inputWidth)) { continue; }
          let pixelBase = (u32(inputY) * params.inputWidth + u32(inputX)) * ${t}u;
          for (var inputBlock = 0u; inputBlock < ${t}u; inputBlock++) {
            ${Ne(
      "inputData[pixelBase + inputBlock]",
      `((((gid.z * 3u + ky) * 3u + kx) * ${t}u + inputBlock) * 4u)`,
      n
    )}
          }
        }
      }
      pooled = max(pooled, ${o});
    }
  }
  let outputIndex = (gid.y * params.outputWidth + gid.x) * ${i}u + gid.z;
  outputData[outputIndex] = ${oe("pooled", n)};
}
`
  );
}
function mo(n, e, t) {
  const i = L(n);
  return (
    /* wgsl */
    `${re(n)}
struct Params {
  inputWidth: u32,
  inputHeight: u32,
  outputWidth: u32,
  outputHeight: u32,
  outputBlocks: u32,
}
@group(0) @binding(0) var<storage, read> inputData: array<${i}>;
@group(0) @binding(1) var<storage, read_write> outputData: array<${i}>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(${D}, ${D}, 1)
fn main(
  @builtin(global_invocation_id) gid: vec3<u32>${t ? `,
  @builtin(num_workgroups) groupCount: vec3<u32>` : ""}
) {
  ${t ? `let linearX = gid.x + gid.z * groupCount.x * ${D}u;
  let outputBlock = linearX % ${e}u;
  let outputX = linearX / ${e}u;` : `let outputBlock = gid.z;
  let outputX = gid.x;`}
  if (
    outputX >= params.outputWidth ||
    gid.y >= params.outputHeight ||
    outputBlock >= ${e}u
  ) {
    return;
  }
  var pooled = vec4<f32>(-3.402823466e+38);
  for (var py = 0u; py < 2u; py++) {
    let inputY = gid.y * 2u + py;
    if (inputY >= params.inputHeight) { continue; }
    for (var px = 0u; px < 2u; px++) {
      let inputX = outputX * 2u + px;
      if (inputX >= params.inputWidth) { continue; }
      let inputIndex =
        (inputY * params.inputWidth + inputX) * ${e}u + outputBlock;
      pooled = max(pooled, vec4<f32>(inputData[inputIndex]));
    }
  }
  let outputIndex =
    (gid.y * params.outputWidth + outputX) * ${e}u + outputBlock;
  outputData[outputIndex] = ${oe("pooled", n)};
}
`
  );
}
function _o(n, e, t, i, r, o, a) {
  const { workgroupX: u, workgroupY: s, rowsPerThread: l, tileM: p, tileNBlocks: c, tileKBlocks: d } = ie(a), { addressMode: h, weightLayout: f } = a, g = L(n), y = L(e), v = Je(n, a, !1), _ = Je(n, a, !0), w = v ? "vec2<u32>" : g, $ = _ ? "vec2<u32>" : g, x = i[0] + i[1], B = oe(
    de("acc[row]", t),
    e
  ), k = u * s, C = p * d, z = d * c * 4, X = (N, ye) => (
    /* wgsl */
    `
          {
            let sourceBlock = ${ye};
            ${h === "base-offset" ? `let dx = ${N === r ? "(i32((cachedMask[load] >> 9u) & 1u) + i32(kernelX) - 1) >> 1u" : "i32(kernelX) - 1"};
            let dy = ${N === r ? "(i32((cachedMask[load] >> 10u) & 1u) + i32(kernelY) - 1) >> 1u" : "i32(kernelY) - 1"};
            let offset = (dy * i32(params.source${N}Width) + dx) * ${i[N]};
            let sourceIndex = cachedBase${N}[load] + u32(offset) + sourceBlock;` : `let sourceX = ${N === r ? "u32(inputX) / 2u" : "u32(inputX)"};
            let sourceY = ${N === r ? "u32(inputY) / 2u" : "u32(inputY)"};
            let sourceIndex =
              (sourceY * params.source${N}Width + sourceX) *
              ${i[N]}u + sourceBlock;`}
            value = ${Ae(`input${N}[sourceIndex]`, v)};
          }`
  );
  return (
    /* wgsl */
    `${re(n)}
struct Params {
  outputWidth: u32,
  outputHeight: u32,
  outputBlocks: u32,
  inputBlocks: u32,
  source0Width: u32,
  source0Height: u32,
  source1Width: u32,
  source1Height: u32,
}
@group(0) @binding(0) var<storage, read> input0: array<${w}>;
@group(0) @binding(1) var<storage, read> input1: array<${w}>;
@group(0) @binding(2) var<storage, read> weights: array<${$}>;
@group(0) @binding(3) var<storage, read> bias: array<vec4<f32>>;
@group(0) @binding(4) var<storage, read_write> outputData: array<${y}>;
@group(0) @binding(5) var<uniform> params: Params;

var<workgroup> inputTile: array<${g}, ${me(a).input}>;
var<workgroup> weightTile: array<${g}, ${me(a).weights}>;

@compute @workgroup_size(${u}, ${s}, 1)
fn main(
  @builtin(local_invocation_id) localId: vec3<u32>,
  @builtin(workgroup_id) workgroupId: vec3<u32>
) {
  let spatialBase =
    workgroupId.x * ${p}u +
    localId.y * ${l}u;
  let outputBlock =
    workgroupId.y * ${c}u + localId.x;
  let spatialCount = params.outputWidth * params.outputHeight;
  var acc: array<vec4<f32>, ${l}>;
  if (outputBlock < ${o}u) {
    for (var row = 0u; row < ${l}u; row++) {
      acc[row] = bias[outputBlock];
    }
  }

  let localLinear =
    localId.y * ${u}u + localId.x;
  ${h !== "analytic" ? fn(x, !0, a, { blocks: i, upsampled: r }) : ""}
  let totalK = ${x * 9}u;
  for (var kBase = 0u; kBase < totalK; kBase += ${d}u) {
    ${h !== "analytic" ? a.decoderLoad === "source-first" ? `
    if (channelBlock < ${i[0]}u) {
      ${Fe(g, X(0, "inputBlock"), a)}
    } else {
      ${Fe(g, X(1, `inputBlock - ${i[0]}u`), a)}
    }
    ` : Fe(g, `
        if (inputBlock < ${i[0]}u) {
          ${X(0, "inputBlock")}
        } else {
          ${X(1, `inputBlock - ${i[0]}u`)}
        }
    `, a) : (
      /* wgsl */
      `
    for (
      var loadIndex = localLinear;
      loadIndex < ${C}u;
      loadIndex += ${k}u
    ) {
      let tileSpatial = loadIndex / ${d}u;
      let tileK = loadIndex % ${d}u;
      let outputSpatialIndex =
        workgroupId.x * ${p}u + tileSpatial;
      let kIndex = kBase + tileK;
      var value = ${g}(0.0);
      if (outputSpatialIndex < spatialCount && kIndex < totalK) {
        let outputY = outputSpatialIndex / params.outputWidth;
        let outputX = outputSpatialIndex % params.outputWidth;
        let inputBlock = kIndex % ${x}u;
        let kernelIndex = kIndex / ${x}u;
        let kernelY = kernelIndex / 3u;
        let kernelX = kernelIndex % 3u;
        let inputY = i32(outputY) + i32(kernelY) - 1;
        let inputX = i32(outputX) + i32(kernelX) - 1;
        if (
          inputY >= 0 && inputY < i32(params.outputHeight) &&
          inputX >= 0 && inputX < i32(params.outputWidth)
        ) {
          if (inputBlock < ${i[0]}u) {
            ${X(0, "inputBlock")}
          } else {
            ${X(1, `inputBlock - ${i[0]}u`)}
          }
        }
      }
      inputTile[${rt("loadIndex", a)}] = value;
    }
    `
    )}

    for (
      var loadIndex = localLinear;
      loadIndex < ${z}u;
      loadIndex += ${k}u
    ) {
      let tileK = loadIndex / ${c * 4}u;
      let outputRemainder = loadIndex % ${c * 4}u;
      let tileOutputBlock = outputRemainder / 4u;
      let outputLane = outputRemainder % 4u;
      let loadedOutputBlock =
        workgroupId.y * ${c}u + tileOutputBlock;
      let kIndex = kBase + tileK;
      var value = ${g}(0.0);
      if (loadedOutputBlock < ${o}u && kIndex < totalK) {
        ${f === "k-major" ? (
      /* wgsl */
      `
        let weightIndex = (kIndex * ${o}u + loadedOutputBlock) * 4u + outputLane;
        `
    ) : h !== "analytic" ? (
      /* wgsl */
      `
        let weightIndex = (loadedOutputBlock * ${x * 9}u + kIndex) * 4u + outputLane;
        `
    ) : (
      /* wgsl */
      `
        let inputBlock = kIndex % ${x}u;
        let kernelIndex = kIndex / ${x}u;
        let kernelY = kernelIndex / 3u;
        let kernelX = kernelIndex % 3u;
        let weightIndex =
          ((((loadedOutputBlock * 3u + kernelY) * 3u + kernelX) *
            ${x}u + inputBlock) * 4u + outputLane);
        `
    )}
        value = ${Ae("weights[weightIndex]", _)};
      }
      weightTile[${pn("loadIndex", a)}] = value;
    }

    workgroupBarrier();
    ${hn(n, a)}
    workgroupBarrier();
    ${h !== "analytic" ? gn(x, a) : ""}
  }

  if (outputBlock < ${o}u) {
    for (var row = 0u; row < ${l}u; row++) {
      let spatialIndex = spatialBase + row;
      if (spatialIndex < spatialCount) {
        let outputIndex = spatialIndex * ${o}u + outputBlock;
        outputData[outputIndex] = ${B};
      }
    }
  }
}
`
  );
}
function yo(n, e, t, i, r, o) {
  const a = L(n), u = L(e), s = i[0] + i[1], l = (c, d) => {
    const h = c === r;
    return (
      /* wgsl */
      `
      {
        let sourceX = ${h ? "u32(inputX) / 2u" : "u32(inputX)"};
        let sourceY = ${h ? "u32(inputY) / 2u" : "u32(inputY)"};
        let sourcePixelBase = (sourceY * params.source${c}Width + sourceX) * ${i[c]}u;
        for (var sourceBlock = 0u; sourceBlock < ${i[c]}u; sourceBlock++) {
          let inputBlock = ${d}u + sourceBlock;
          ${Ne(
        `input${c}[sourcePixelBase + sourceBlock]`,
        `((((gid.z * 3u + ky) * 3u + kx) * ${s}u + inputBlock) * 4u)`,
        n
      )}
        }
      }
`
    );
  }, p = oe(
    de("acc", t),
    e
  );
  return (
    /* wgsl */
    `${re(n)}
struct Params {
  outputWidth: u32,
  outputHeight: u32,
  outputBlocks: u32,
  inputBlocks: u32,
  source0Width: u32,
  source0Height: u32,
  source1Width: u32,
  source1Height: u32,
}
@group(0) @binding(0) var<storage, read> input0: array<${a}>;
@group(0) @binding(1) var<storage, read> input1: array<${a}>;
@group(0) @binding(2) var<storage, read> weights: array<${a}>;
@group(0) @binding(3) var<storage, read> bias: array<vec4<f32>>;
@group(0) @binding(4) var<storage, read_write> outputData: array<${u}>;
@group(0) @binding(5) var<uniform> params: Params;

@compute @workgroup_size(${D}, ${D}, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= params.outputWidth || gid.y >= params.outputHeight || gid.z >= ${o}u) {
    return;
  }
  var acc = bias[gid.z];
  for (var ky = 0u; ky < 3u; ky++) {
    let inputY = i32(gid.y) + i32(ky) - 1;
    if (inputY < 0 || inputY >= i32(params.outputHeight)) { continue; }
    for (var kx = 0u; kx < 3u; kx++) {
      let inputX = i32(gid.x) + i32(kx) - 1;
      if (inputX < 0 || inputX >= i32(params.outputWidth)) { continue; }
      ${l(0, 0)}
      ${l(1, i[0])}
    }
  }
  let outputIndex = (gid.y * params.outputWidth + gid.x) * ${o}u + gid.z;
  outputData[outputIndex] = ${p};
}
`
  );
}
function wo(n, e, t, i, r, o) {
  const a = L(n), u = L(e), s = i[0] + i[1], l = q * q * s, p = U * U, c = oe(
    de("acc", t),
    e
  ), d = (h, f) => (
    /* wgsl */
    `
      let sourceBlock = ${f};
      let sourceIndex =
        (${h === r ? "u32(inputY) / 2u" : "u32(inputY)"} * params.source${h}Width + ${h === r ? "u32(inputX) / 2u" : "u32(inputX)"}) *
        ${i[h]}u + sourceBlock;
      value = input${h}[sourceIndex];`
  );
  return (
    /* wgsl */
    `${re(n)}
struct Params {
  outputWidth: u32,
  outputHeight: u32,
  outputBlocks: u32,
  inputBlocks: u32,
  source0Width: u32,
  source0Height: u32,
  source1Width: u32,
  source1Height: u32,
}
@group(0) @binding(0) var<storage, read> input0: array<${a}>;
@group(0) @binding(1) var<storage, read> input1: array<${a}>;
@group(0) @binding(2) var<storage, read> weights: array<${a}>;
@group(0) @binding(3) var<storage, read> bias: array<vec4<f32>>;
@group(0) @binding(4) var<storage, read_write> outputData: array<${u}>;
@group(0) @binding(5) var<uniform> params: Params;

var<workgroup> inputPatch: array<${a}, ${l}>;

@compute @workgroup_size(${U}, ${U}, 1)
fn main(
  @builtin(local_invocation_id) localId: vec3<u32>,
  @builtin(workgroup_id) workgroupId: vec3<u32>
) {
  let localLinear =
    localId.y * ${U}u + localId.x;
  for (
    var loadIndex = localLinear;
    loadIndex < ${l}u;
    loadIndex += ${p}u
  ) {
    let patchPixel = loadIndex / ${s}u;
    let inputBlock = loadIndex % ${s}u;
    let patchX = patchPixel % ${q}u;
    let patchY = patchPixel / ${q}u;
    let inputX =
      i32(workgroupId.x * ${U}u + patchX) - 1;
    let inputY =
      i32(workgroupId.y * ${U}u + patchY) - 1;
    var value = ${a}(0.0);
    if (
      inputX >= 0 && inputX < i32(params.outputWidth) &&
      inputY >= 0 && inputY < i32(params.outputHeight)
    ) {
      if (inputBlock < ${i[0]}u) {
        ${d(0, "inputBlock")}
      } else {
        ${d(1, `inputBlock - ${i[0]}u`)}
      }
    }
    inputPatch[loadIndex] = value;
  }
  workgroupBarrier();

  let outputX =
    workgroupId.x * ${U}u + localId.x;
  let outputY =
    workgroupId.y * ${U}u + localId.y;
  let outputBlock = workgroupId.z;
  if (
    outputX >= params.outputWidth || outputY >= params.outputHeight ||
    outputBlock >= ${o}u
  ) {
    return;
  }

  var acc = bias[outputBlock];
  for (var ky = 0u; ky < 3u; ky++) {
    for (var kx = 0u; kx < 3u; kx++) {
      let patchBase =
        ((localId.y + ky) * ${q}u + localId.x + kx) *
        ${s}u;
      for (var inputBlock = 0u; inputBlock < ${s}u; inputBlock++) {
        ${Ne(
      "inputPatch[patchBase + inputBlock]",
      `((((outputBlock * 3u + ky) * 3u + kx) * ${s}u + inputBlock) * 4u)`,
      n
    )}
      }
    }
  }
  let outputIndex =
    (outputY * params.outputWidth + outputX) * ${o}u + outputBlock;
  outputData[outputIndex] = ${c};
}
`
  );
}
function li(n, e) {
  const t = L(n), i = Array.from(
    { length: e },
    (u, s) => `@group(0) @binding(${s}) var<storage, read> input${s}: array<vec4<f32>>;`
  ).join(`
`), r = Array.from({ length: e }, (u, s) => {
    const l = s * 3;
    return `if (channel < ${l + 3}u) { return input${s}[pixel][channel - ${l}u]; }`;
  }).join(`
  `), o = e, a = e + 1;
  return (
    /* wgsl */
    `${re(n)}
struct Params {
  width: u32,
  height: u32,
  outputBlocks: u32,
  inputChannels: u32,
}
${i}
@group(0) @binding(${o}) var<storage, read_write> outputData: array<${t}>;
@group(0) @binding(${a}) var<uniform> params: Params;

fn readChannel(pixel: u32, channel: u32) -> f32 {
  ${r}
  return 0.0;
}

@compute @workgroup_size(${D}, ${D}, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= params.width || gid.y >= params.height || gid.z >= params.outputBlocks) {
    return;
  }
  let pixel = gid.y * params.width + gid.x;
  let firstChannel = gid.z * 4u;
  let value = vec4<f32>(
    readChannel(pixel, firstChannel),
    readChannel(pixel, firstChannel + 1u),
    readChannel(pixel, firstChannel + 2u),
    readChannel(pixel, firstChannel + 3u)
  );
  outputData[pixel * params.outputBlocks + gid.z] = ${oe("value", n)};
}
`
  );
}
function vo(n) {
  return n.op === "concat" ? n.inputs : n.op === "fusedUpsampleConcatConv2d" ? n.inputs.map((e) => e.value) : [n.input];
}
function xo(n, e = "auto") {
  const t = n.features.has("shader-f16");
  if (e === "fp16" && !t)
    throw new Error(
      "OIDN FP16 was requested but the GPUDevice does not have shader-f16 enabled"
    );
  return e === "auto" ? t ? "fp16" : "fp32" : e;
}
class $o {
  constructor(e, t, i = {}) {
    m(this, "precision");
    m(this, "kernelSetting");
    m(this, "gemm");
    m(this, "maxSpatialInputBlocks");
    m(this, "subgroupsAvailable");
    m(this, "_model");
    m(this, "_gemmByOutputBlocks", /* @__PURE__ */ new Map());
    m(this, "_packedConvs", /* @__PURE__ */ new Map());
    m(this, "_pipelineCache");
    m(this, "_pipelinePromises");
    m(this, "_executionCache", /* @__PURE__ */ new Map());
    m(this, "_retiredExecutions", /* @__PURE__ */ new Set());
    m(this, "_clock", 0);
    m(this, "_shapeCacheSize");
    m(this, "_profileNextExecution", !1);
    m(this, "_lastExecutionProfile");
    m(this, "_profileOperations", 0);
    m(this, "_resources", new cn());
    m(this, "_disposed", !1);
    var s, l, p, c, d, h, f, g, y, v, _, w;
    this._device = e;
    const r = oo(e);
    this._pipelineCache = r.ready, this._pipelinePromises = r.pending, this.precision = xo(
      e,
      i.precision ?? "auto"
    ), this.kernelSetting = i.kernel ?? "auto";
    const o = ((s = i.gemm) == null ? void 0 : s.workgroupSize) ?? [8, 8];
    if (this.gemm = Object.freeze({
      tilePolicy: ((l = i.gemm) == null ? void 0 : l.tilePolicy) ?? ((p = i.gemm) != null && p.workgroupSize ? "fixed" : "output-aligned"),
      decoderLoad: ((c = i.gemm) == null ? void 0 : c.decoderLoad) ?? "per-load",
      poolLayout: ((d = i.gemm) == null ? void 0 : d.poolLayout) ?? "channels",
      sharedLayout: ((h = i.gemm) == null ? void 0 : h.sharedLayout) ?? (this.precision === "fp16" ? "padded-input" : "padded"),
      accumulationOrder: ((f = i.gemm) == null ? void 0 : f.accumulationOrder) ?? "k-major",
      finalLayer: ((g = i.gemm) == null ? void 0 : g.finalLayer) ?? "shared-auto",
      loadMode: ((y = i.gemm) == null ? void 0 : y.loadMode) ?? "native",
      addressMode: ((v = i.gemm) == null ? void 0 : v.addressMode) ?? "incremental",
      weightLayout: ((_ = i.gemm) == null ? void 0 : _.weightLayout) ?? "k-major",
      rowsPerThread: ((w = i.gemm) == null ? void 0 : w.rowsPerThread) ?? 8,
      workgroupSize: Object.freeze([o[0], o[1]])
    }), !["fixed", "output-aligned"].includes(this.gemm.tilePolicy))
      throw new Error(`Unsupported GEMM tile policy: ${this.gemm.tilePolicy}`);
    if (!["per-load", "source-first"].includes(this.gemm.decoderLoad))
      throw new Error(`Unsupported GEMM decoder load: ${this.gemm.decoderLoad}`);
    if (!["spatial", "channels"].includes(this.gemm.poolLayout))
      throw new Error(`Unsupported GEMM pool layout: ${this.gemm.poolLayout}`);
    if (!["linear", "padded", "padded-input", "padded-weights"].includes(this.gemm.sharedLayout))
      throw new Error(`Unsupported GEMM shared layout: ${this.gemm.sharedLayout}`);
    if (!["k-major", "row-major"].includes(this.gemm.accumulationOrder))
      throw new Error(`Unsupported GEMM accumulation order: ${this.gemm.accumulationOrder}`);
    if (!["direct", "shared-input", "shared-input-weights", "shared-auto"].includes(this.gemm.finalLayer))
      throw new Error(`Unsupported GEMM final layer: ${this.gemm.finalLayer}`);
    if (!["native", "packed-weights", "packed-all"].includes(this.gemm.loadMode))
      throw new Error(`Unsupported GEMM load mode: ${this.gemm.loadMode}`);
    if (!["analytic", "incremental", "base-offset"].includes(this.gemm.addressMode))
      throw new Error(`Unsupported GEMM address mode: ${this.gemm.addressMode}`);
    if (!["output-major", "k-major"].includes(this.gemm.weightLayout))
      throw new Error(`Unsupported GEMM weight layout: ${this.gemm.weightLayout}`);
    const a = ie(this.gemm), u = (me(this.gemm).input + me(this.gemm).weights) * 4 * (this.precision === "fp16" ? 2 : 4);
    if (![2, 4, 8].includes(a.rowsPerThread) || ![4, 8, 16].includes(a.workgroupX) || ![4, 8].includes(a.workgroupY) || o.length !== 2 || a.workgroupX > e.limits.maxComputeWorkgroupSizeX || a.workgroupY > e.limits.maxComputeWorkgroupSizeY || a.workgroupX * a.workgroupY > e.limits.maxComputeInvocationsPerWorkgroup || u > e.limits.maxComputeWorkgroupStorageSize)
      throw new Error("Unsupported GEMM tile configuration for this GPUDevice");
    if (this.subgroupsAvailable = e.features.has(
      "subgroups"
    ), this.maxSpatialInputBlocks = this.precision === "fp16" && e.limits.maxComputeInvocationsPerWorkgroup >= U * U && e.limits.maxComputeWorkgroupSizeX >= U && e.limits.maxComputeWorkgroupSizeY >= U ? Math.floor(
      e.limits.maxComputeWorkgroupStorageSize / (q * q * 4 * 2)
    ) : 0, this._shapeCacheSize = Math.max(1, i.shapeCacheSize ?? 2), t.inputChannels % 3 !== 0 || t.inputChannels < 3 || t.inputChannels > 9)
      throw new Error(
        `Native OIDN expects 3, 6, or 9 input channels, got ${t.inputChannels}`
      );
    this._model = {
      spec: t.spec,
      inputChannels: t.inputChannels,
      outputChannels: t.outputChannels,
      channelsByValue: new Map(t.channelsByValue),
      convChannels: new Map(t.convChannels)
    };
    for (const $ of kt(this._model, {
      fuseConvPool: !1
    }).nodes)
      if ($.op !== "conv2d" && $.op !== "maxPool2d" && $.op !== "fusedConvReluMaxPool2d" && $.op !== "fusedUpsampleConcatConv2d")
        throw new Error(
          `Native OIDN descriptor ${t.spec.id} leaves unsupported ${$.op} node ${$.id} after graph optimization`
        );
    try {
      for (const [$, x] of t.convTensors) {
        const k = this._selectConvKernel(
          M(x.inputChannels),
          $ === t.spec.output
        ) === "implicit-gemm" ? this.gemm.weightLayout : "output-major", C = uo(
          e,
          $,
          x,
          this.precision,
          k
        );
        this._resources.track("gpu-buffer", C.weights), this._resources.track("gpu-buffer", C.bias), this._packedConvs.set($, C);
      }
    } catch ($) {
      for (const x of this._packedConvs.values())
        this._releaseBuffer(x.weights), this._releaseBuffer(x.bias);
      throw this._packedConvs.clear(), $;
    }
  }
  _pipeline(e, t) {
    let i = this._pipelineCache.get(e);
    return i || (i = this._device.createComputePipeline({
      label: `oidn/${e}`,
      layout: "auto",
      compute: {
        module: this._device.createShaderModule({
          label: `oidn/${e}`,
          code: t
        }),
        entryPoint: "main"
      }
    }), this._pipelineCache.set(e, i)), i;
  }
  _pipelineAsync(e, t) {
    const i = this._pipelineCache.get(e);
    if (i) return Promise.resolve(i);
    const r = this._pipelinePromises.get(e);
    if (r) return r;
    const o = this._device.createComputePipelineAsync({
      label: `oidn/${e}`,
      layout: "auto",
      compute: {
        module: this._device.createShaderModule({
          label: `oidn/${e}`,
          code: t
        }),
        entryPoint: "main"
      }
    }).then((a) => (this._pipelineCache.set(e, a), this._pipelinePromises.delete(e), a), (a) => {
      throw this._pipelinePromises.delete(e), a;
    });
    return this._pipelinePromises.set(e, o), o;
  }
  _nodePipelineSpec(e, t) {
    if (e.op === "conv2d") {
      const i = t ? "fp32" : this.precision, r = M(
        this._model.convChannels.get(e.id).inputChannels
      ), o = M(
        this._model.convChannels.get(e.id).outputChannels
      ), a = this._gemmForOutput(o), u = this._selectConvKernel(r, t), s = a.finalLayer === "shared-input-weights" || a.finalLayer === "shared-auto" && oi(this.precision, r, !0) <= this._device.limits.maxComputeWorkgroupStorageSize;
      return t && this._model.convChannels.get(e.id).outputChannels === 3 && (this.kernelSetting === "auto" || this.kernelSetting === "implicit-gemm") && a.finalLayer !== "direct" && this._device.limits.maxComputeWorkgroupSizeX >= 8 && this._device.limits.maxComputeWorkgroupSizeY >= 8 && this._device.limits.maxComputeInvocationsPerWorkgroup >= 64 && oi(this.precision, r, s) <= this._device.limits.maxComputeWorkgroupStorageSize ? {
        key: `conv-final-rgb/${this.precision}/${e.activation}/in${r}/weights-${s ? "shared" : "storage"}`,
        kernel: "direct",
        code: eo(this.precision, e.activation, r, s)
      } : {
        key: `conv-${u}/${this.precision}/${i}/${e.activation}/in${r}/out${o}` + (u === "implicit-gemm" ? `/address-${a.addressMode}/weights-${a.weightLayout}-v1/tile-${a.workgroupSize.join("x")}-r${a.rowsPerThread}/loads-${a.loadMode}/shared-${a.sharedLayout}/acc-${a.accumulationOrder}` : ""),
        kernel: u,
        code: u === "implicit-gemm" ? fo(
          this.precision,
          i,
          e.activation,
          r,
          o,
          a
        ) : u === "spatial" ? ho(
          this.precision,
          i,
          e.activation,
          r,
          o
        ) : u === "subgroup" ? po(
          this.precision,
          i,
          e.activation,
          r,
          o
        ) : lo(
          this.precision,
          i,
          e.activation,
          r,
          o
        )
      };
    }
    if (e.op === "maxPool2d") {
      const i = M(
        this._model.channelsByValue.get(e.id)
      ), r = this._coalescedPool();
      return {
        key: `max-pool/${this.precision}/out${i}/${r ? "channels" : "spatial"}`,
        kernel: "direct",
        code: mo(this.precision, i, r)
      };
    }
    if (e.op === "fusedConvReluMaxPool2d") {
      const i = M(
        this._model.convChannels.get(e.conv.id).inputChannels
      ), r = M(
        this._model.convChannels.get(e.conv.id).outputChannels
      );
      return {
        key: `conv-pool/${this.precision}/${e.conv.activation}/in${i}/out${r}`,
        kernel: "direct",
        code: go(
          this.precision,
          e.conv.activation,
          i,
          r
        )
      };
    }
    if (e.op === "fusedUpsampleConcatConv2d") {
      if (e.inputs.length !== 2)
        throw new Error(`Native fused decoder ${e.id} requires two inputs`);
      const i = e.inputs.map(
        (d) => M(this._model.channelsByValue.get(d.value))
      ), r = e.inputs.findIndex((d) => d.upsample);
      if (r !== 0 && r !== 1)
        throw new Error(`Native fused decoder ${e.id} has no upsample input`);
      const o = i[0] + i[1], a = t ? "fp32" : this.precision, u = this._selectConvKernel(o, t), s = u === "subgroup" ? "direct" : u, l = M(
        this._model.convChannels.get(e.conv.id).outputChannels
      ), p = this._gemmForOutput(l);
      return {
        key: `decoder-${s}/${this.precision}/${a}/${e.conv.activation}/${i.join("+")}/out${l}/up${r}` + (s === "implicit-gemm" ? `/address-${p.addressMode}/weights-${p.weightLayout}-v1/tile-${p.workgroupSize.join("x")}-r${p.rowsPerThread}/loads-${p.loadMode}/shared-${p.sharedLayout}/acc-${p.accumulationOrder}/decoder-${p.decoderLoad}` : ""),
        kernel: s,
        code: s === "implicit-gemm" ? _o(
          this.precision,
          a,
          e.conv.activation,
          i,
          r,
          l,
          p
        ) : s === "spatial" ? wo(
          this.precision,
          a,
          e.conv.activation,
          i,
          r,
          l
        ) : yo(
          this.precision,
          a,
          e.conv.activation,
          i,
          r,
          l
        )
      };
    }
    throw new Error(
      `Native OIDN does not implement unfused ${e.op} node ${e.id}`
    );
  }
  // Wider output tiles reuse each input load across twice as many channels.
  // Require full output blocks and keep the validated base tile as fallback.
  _gemmForOutput(e) {
    if (this.gemm.tilePolicy !== "output-aligned" || this.gemm.workgroupSize[0] !== 8 || e <= 0 || e % 16 !== 0)
      return this.gemm;
    const t = this._gemmByOutputBlocks.get(e);
    if (t) return t;
    const i = Object.freeze({
      ...this.gemm,
      workgroupSize: Object.freeze([16, this.gemm.workgroupSize[1]])
    }), r = ie(i), o = me(i), a = this._device.limits, s = r.workgroupX <= a.maxComputeWorkgroupSizeX && r.workgroupY <= a.maxComputeWorkgroupSizeY && r.workgroupX * r.workgroupY <= a.maxComputeInvocationsPerWorkgroup && (o.input + o.weights) * 4 * (this.precision === "fp16" ? 2 : 4) <= a.maxComputeWorkgroupStorageSize ? i : this.gemm;
    return this._gemmByOutputBlocks.set(e, s), s;
  }
  _coalescedPool() {
    return this.gemm.poolLayout === "channels" && (this.kernelSetting === "auto" || this.kernelSetting === "implicit-gemm");
  }
  _selectConvKernel(e, t) {
    const i = this.precision === "fp16" && e <= this.maxSpatialInputBlocks;
    return this.kernelSetting === "direct" ? "direct" : this.kernelSetting === "spatial" ? i ? "spatial" : "direct" : this.kernelSetting === "implicit-gemm" ? t ? "direct" : "implicit-gemm" : this.kernelSetting === "subgroup" ? this.subgroupsAvailable ? "subgroup" : "direct" : t ? "direct" : "implicit-gemm";
  }
  _nodePipeline(e, t) {
    const { key: i, code: r } = this._nodePipelineSpec(e, t);
    return this._pipeline(i, r);
  }
  /** Compiles all shape-independent kernels before the model reports ready. */
  async prepare() {
    if (this._disposed) throw new Error("Native OIDN executor is disposed");
    const e = kt(this._model, { fuseConvPool: !1 }), t = this._model.inputChannels / 3, i = [
      {
        key: `pack/${this.precision}/${t}`,
        code: li(this.precision, t)
      },
      ...e.nodes.map(
        (r) => this._nodePipelineSpec(r, r.id === e.spec.output)
      )
    ];
    await Promise.all(
      i.map(({ key: r, code: o }) => this._pipelineAsync(r, o))
    );
  }
  _createExecution(e, t) {
    const i = ro(this._model, e, t, {
      fuseConvPool: !1
    }), r = /* @__PURE__ */ new Map(), o = [], a = /* @__PURE__ */ new Map();
    i.nodes.forEach((l, p) => {
      for (const c of vo(l)) a.set(c, p);
    }), a.set(i.spec.output, i.nodes.length);
    const u = [], s = (l) => (u.push(l), this._resources.track("gpu-buffer", l));
    try {
      const l = (_, w, $, x) => {
        for (const C of o)
          C.activeValue && (a.get(C.activeValue) ?? -1) < x && (C.activeValue = void 0);
        const B = ao(w, $);
        let k = o.filter((C) => !C.activeValue && C.capacity >= B).sort((C, z) => C.capacity - z.capacity)[0];
        k || (k = { buffer: s(this._device.createBuffer({
          label: `oidn/activation/${e}x${t}/${o.length}`,
          size: Ot(B, 4),
          usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC | GPUBufferUsage.COPY_DST
        })), capacity: B }, o.push(k)), k.activeValue = _, r.set(_, k.buffer);
      };
      l(
        i.spec.input,
        i.inputShape,
        this.precision === "fp16" ? 2 : 4,
        -1
      ), i.plannedNodes.forEach(({ node: _, outputShape: w }, $) => {
        const x = _.id === i.spec.output;
        l(
          _.id,
          w,
          x || this.precision === "fp32" ? 4 : 2,
          $
        );
      });
      const p = this._model.inputChannels / 3, c = `pack/${this.precision}/${p}`, d = this._pipeline(
        c,
        li(this.precision, p)
      ), h = [], f = [], g = [], y = [];
      i.plannedNodes.forEach(({ node: _, outputShape: w }, $) => {
        const x = _.id === i.spec.output, B = this._nodePipelineSpec(_, x), k = this._pipeline(B.key, B.code);
        h.push(k), f.push(B.kernel ?? "direct");
        const C = r.get(_.id);
        let z, X, N;
        if (_.op === "conv2d") {
          const R = i.valueShapes.get(_.input);
          N = _.id, X = [
            R.width,
            R.height,
            w.width,
            w.height,
            M(R.channels),
            M(w.channels)
          ];
          const j = this._packedConvs.get(N);
          z = [
            { binding: 0, resource: { buffer: r.get(_.input) } },
            { binding: 1, resource: { buffer: j.weights } },
            { binding: 2, resource: { buffer: j.bias } },
            { binding: 3, resource: { buffer: C } }
          ];
        } else if (_.op === "maxPool2d") {
          const R = i.valueShapes.get(_.input);
          X = [
            R.width,
            R.height,
            w.width,
            w.height,
            M(w.channels)
          ], z = [
            { binding: 0, resource: { buffer: r.get(_.input) } },
            { binding: 1, resource: { buffer: C } }
          ];
        } else if (_.op === "fusedConvReluMaxPool2d") {
          const R = i.valueShapes.get(_.input);
          N = _.conv.id, X = [
            R.width,
            R.height,
            w.width,
            w.height,
            M(R.channels),
            M(w.channels)
          ];
          const j = this._packedConvs.get(N);
          z = [
            { binding: 0, resource: { buffer: r.get(_.input) } },
            { binding: 1, resource: { buffer: j.weights } },
            { binding: 2, resource: { buffer: j.bias } },
            { binding: 3, resource: { buffer: C } }
          ];
        } else if (_.op === "fusedUpsampleConcatConv2d") {
          N = _.conv.id;
          const R = i.valueShapes.get(_.inputs[0].value), j = i.valueShapes.get(_.inputs[1].value);
          X = [
            w.width,
            w.height,
            M(w.channels),
            M(this._model.convChannels.get(N).inputChannels),
            R.width,
            R.height,
            j.width,
            j.height
          ];
          const Be = this._packedConvs.get(N);
          z = [
            {
              binding: 0,
              resource: { buffer: r.get(_.inputs[0].value) }
            },
            {
              binding: 1,
              resource: { buffer: r.get(_.inputs[1].value) }
            },
            { binding: 2, resource: { buffer: Be.weights } },
            { binding: 3, resource: { buffer: Be.bias } },
            { binding: 4, resource: { buffer: C } }
          ];
        } else
          throw new Error(`Unexpected native node ${_.op}`);
        const ye = s(
          ci(
            this._device,
            `oidn/${_.id}/params/${e}x${t}`,
            X
          )
        );
        y.push(ye), z.push({ binding: z.length, resource: { buffer: ye } }), g.push(
          this._device.createBindGroup({
            label: `oidn/${_.id}/bindings`,
            layout: k.getBindGroupLayout(0),
            entries: z
          })
        );
      });
      const v = s(
        ci(
          this._device,
          `oidn/input/params/${e}x${t}`,
          [
            e,
            t,
            M(this._model.inputChannels),
            this._model.inputChannels
          ]
        )
      );
      return y.push(v), {
        plan: i,
        valueBuffers: r,
        slots: o,
        nodeBindings: g,
        nodePipelines: h,
        nodeKernels: f,
        inputPipeline: d,
        inputUniform: v,
        ownedBuffers: y,
        lastUsed: ++this._clock
      };
    } catch (l) {
      for (const p of u) this._releaseBuffer(p);
      throw l;
    }
  }
  _execution(e, t) {
    if (this._disposed) throw new Error("Native OIDN executor is disposed");
    const i = `${e}x${t}`;
    let r = this._executionCache.get(i);
    if (!r && (r = this._createExecution(e, t), this._executionCache.set(i, r), this._executionCache.size > this._shapeCacheSize)) {
      const o = [...this._executionCache.entries()].filter(([a]) => a !== i).sort((a, u) => a[1].lastUsed - u[1].lastUsed)[0];
      o && (this._executionCache.delete(o[0]), this._retiredExecutions.add(o[1]), this._device.queue.onSubmittedWorkDone().catch(() => {
      }).then(() => {
        this._retiredExecutions.delete(o[1]), this._destroyExecution(o[1]);
      }));
    }
    return r.lastUsed = ++this._clock, r;
  }
  /** Allocates shape-dependent execution resources before interactive use. */
  prewarm(e) {
    for (const t of e)
      this._execution(t.width, t.height);
  }
  /** Captures per-pass GPU timestamps for the next execute call when supported. */
  profileNextExecution() {
    return this._device.features.has("timestamp-query") ? (this._profileNextExecution = !0, !0) : !1;
  }
  getLastExecutionProfile() {
    return this._lastExecutionProfile;
  }
  execute(e, t, i) {
    const r = this._model.inputChannels / 3;
    if (e.length !== r)
      throw new Error(
        `Native OIDN expected ${r} input buffers, got ${e.length}`
      );
    const o = this._execution(t, i), a = [
      "input-pack",
      ...o.plan.nodes.map((f) => f.id)
    ], u = this._profileNextExecution && this._device.features.has("timestamp-query");
    this._profileNextExecution = !1;
    const s = a.length * 2, l = u ? this._resources.track(
      "gpu-query-set",
      this._device.createQuerySet({ type: "timestamp", count: s })
    ) : void 0, p = s * 8;
    let c, d;
    try {
      c = u ? this._resources.track(
        "gpu-buffer",
        this._device.createBuffer({
          label: `oidn/profile/resolve/${t}x${i}`,
          size: p,
          usage: GPUBufferUsage.QUERY_RESOLVE | GPUBufferUsage.COPY_SRC
        })
      ) : void 0, d = u ? this._resources.track(
        "gpu-buffer",
        this._device.createBuffer({
          label: `oidn/profile/readback/${t}x${i}`,
          size: p,
          usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ
        })
      ) : void 0;
    } catch (f) {
      throw this._releaseBuffer(d), this._releaseBuffer(c), this._releaseQuerySet(l), f;
    }
    const h = (f, g) => ({
      label: f,
      ...l ? {
        timestampWrites: {
          querySet: l,
          beginningOfPassWriteIndex: g * 2,
          endOfPassWriteIndex: g * 2 + 1
        }
      } : {}
    });
    try {
      const f = this._device.createCommandEncoder({
        label: `oidn/native/${t}x${i}`
      }), g = e.map(
        (v, _) => ({ binding: _, resource: { buffer: v } })
      );
      g.push({
        binding: r,
        resource: { buffer: o.valueBuffers.get(o.plan.spec.input) }
      }), g.push({
        binding: r + 1,
        resource: { buffer: o.inputUniform }
      });
      const y = this._device.createBindGroup({
        label: "oidn/input/bindings",
        layout: o.inputPipeline.getBindGroupLayout(0),
        entries: g
      });
      {
        const v = f.beginComputePass(
          h("oidn/input-pack", 0)
        );
        v.setPipeline(o.inputPipeline), v.setBindGroup(0, y), v.dispatchWorkgroups(
          Math.ceil(t / D),
          Math.ceil(i / D),
          M(this._model.inputChannels)
        ), v.end();
      }
      o.plan.plannedNodes.forEach(({ node: v, outputShape: _ }, w) => {
        const $ = f.beginComputePass(
          h(`oidn/${o.plan.nodes[w].id}`, w + 1)
        );
        if ($.setPipeline(o.nodePipelines[w]), $.setBindGroup(0, o.nodeBindings[w]), v.op === "maxPool2d" && this._coalescedPool()) {
          const x = Math.ceil(
            _.width * M(_.channels) / D
          ), B = this._device.limits.maxComputeWorkgroupsPerDimension;
          $.dispatchWorkgroups(
            Math.min(x, B),
            Math.ceil(_.height / D),
            Math.ceil(x / B)
          );
        } else if (o.nodeKernels[w] === "implicit-gemm") {
          const x = ie(this._gemmForOutput(M(_.channels)));
          $.dispatchWorkgroups(
            Math.ceil(
              _.width * _.height / x.tileM
            ),
            Math.ceil(
              M(_.channels) / x.tileNBlocks
            ),
            1
          );
        } else
          $.dispatchWorkgroups(
            Math.ceil(_.width / D),
            Math.ceil(_.height / D),
            M(_.channels)
          );
        $.end();
      }), l && (f.resolveQuerySet(
        l,
        0,
        s,
        c,
        0
      ), f.copyBufferToBuffer(
        c,
        0,
        d,
        0,
        p
      )), this._device.queue.submit([f.finish()]);
    } catch (f) {
      throw this._releaseBuffer(d), this._releaseBuffer(c), this._releaseQuerySet(l), f;
    }
    return l && (this._profileOperations++, this._lastExecutionProfile = (async () => {
      try {
        await d.mapAsync(GPUMapMode.READ);
        const f = new BigUint64Array(
          d.getMappedRange()
        ), g = a.map((y, v) => ({
          id: y,
          durationMs: Number(f[v * 2 + 1] - f[v * 2]) / 1e6
        }));
        return {
          totalMs: g.reduce(
            (y, v) => y + v.durationMs,
            0
          ),
          layers: g
        };
      } finally {
        d.mapState === "mapped" && d.unmap(), this._releaseQuerySet(l), this._releaseBuffer(c), this._releaseBuffer(d), this._profileOperations--;
      }
    })()), o.valueBuffers.get(o.plan.spec.output);
  }
  /** Executes interleaved CPU image data through the native GPU runtime. */
  async executeCPU(e, t, i) {
    const r = t * i * this._model.inputChannels;
    if (e.length !== r)
      throw new Error(
        `Native OIDN CPU input has ${e.length} values, expected ${r}`
      );
    const o = this._execution(t, i), a = this._model.inputChannels / 3, u = t * i;
    o.cpuInputBuffers || (o.cpuInputBuffers = Array.from({ length: a }, (d, h) => {
      const f = this._device.createBuffer({
        label: `oidn/cpu-input/${t}x${i}/${h}`,
        size: u * 16,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST
      });
      return this._resources.track("gpu-buffer", f), o.ownedBuffers.push(f), f;
    }), o.cpuReadbackBuffer = this._device.createBuffer({
      label: `oidn/cpu-readback/${t}x${i}`,
      size: u * 16,
      usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ
    }), this._resources.track("gpu-buffer", o.cpuReadbackBuffer), o.ownedBuffers.push(o.cpuReadbackBuffer));
    for (let d = 0; d < a; d++) {
      const h = new Float32Array(u * 4);
      for (let f = 0; f < u; f++) {
        const g = f * this._model.inputChannels + d * 3, y = f * 4;
        h[y] = e[g], h[y + 1] = e[g + 1], h[y + 2] = e[g + 2];
      }
      this._device.queue.writeBuffer(
        o.cpuInputBuffers[d],
        0,
        h
      );
    }
    const s = this.execute(o.cpuInputBuffers, t, i), l = this._device.createCommandEncoder({
      label: `oidn/cpu-readback/${t}x${i}`
    });
    l.copyBufferToBuffer(
      s,
      0,
      o.cpuReadbackBuffer,
      0,
      u * 16
    ), this._device.queue.submit([l.finish()]), await o.cpuReadbackBuffer.mapAsync(GPUMapMode.READ);
    const p = new Float32Array(
      o.cpuReadbackBuffer.getMappedRange()
    ), c = new Float32Array(u * 3);
    for (let d = 0; d < u; d++)
      c[d * 3] = p[d * 4], c[d * 3 + 1] = p[d * 4 + 1], c[d * 3 + 2] = p[d * 4 + 2];
    return o.cpuReadbackBuffer.unmap(), c;
  }
  _releaseBuffer(e) {
    this._resources.release("gpu-buffer", e, () => e.destroy());
  }
  _releaseQuerySet(e) {
    this._resources.release(
      "gpu-query-set",
      e,
      () => e.destroy()
    );
  }
  _destroyExecution(e) {
    e.slots.forEach((t) => this._releaseBuffer(t.buffer)), e.ownedBuffers.forEach((t) => this._releaseBuffer(t));
  }
  getResourceInfo() {
    return this._resources.snapshot(
      this._retiredExecutions.size + this._profileOperations
    );
  }
  dispose() {
    if (!this._disposed) {
      this._disposed = !0;
      for (const e of this._packedConvs.values())
        this._releaseBuffer(e.weights), this._releaseBuffer(e.bias);
      this._packedConvs.clear();
      for (const e of this._executionCache.values())
        this._destroyExecution(e);
      this._executionCache.clear();
      for (const e of this._retiredExecutions)
        this._destroyExecution(e);
      this._retiredExecutions.clear();
    }
  }
}
const pi = /* @__PURE__ */ new WeakMap();
function bo(n, e) {
  let t = pi.get(n);
  t || (t = /* @__PURE__ */ new Map(), pi.set(n, t));
  let i = t.get(e);
  if (!i) {
    const r = n.createShaderModule({
      label: `oidn/webnn/input-pack/${e}`,
      code: Bo(e)
    }), o = n.createShaderModule({
      label: "oidn/webnn/output-unpack",
      code: To()
    });
    i = {
      input: n.createComputePipeline({
        label: `oidn/webnn/input-pack/${e}`,
        layout: "auto",
        compute: { module: r, entryPoint: "main" }
      }),
      output: n.createComputePipeline({
        label: "oidn/webnn/output-unpack",
        layout: "auto",
        compute: { module: o, entryPoint: "main" }
      })
    }, t.set(e, i);
  }
  return i;
}
const pe = 8;
function mn(n, e) {
  return Math.ceil(n / e) * e;
}
function ko(n, e, t, i) {
  const r = n.createBuffer({
    label: e,
    size: mn(t.byteLength, 4),
    usage: i,
    mappedAtCreation: !0
  });
  return new Uint8Array(r.getMappedRange()).set(
    new Uint8Array(t.buffer, t.byteOffset, t.byteLength)
  ), r.unmap(), r;
}
function di(n, e, t) {
  const i = new Uint32Array(mn(t.length, 4));
  return i.set(t), ko(n, e, i, GPUBufferUsage.UNIFORM);
}
function gt(n, e, t, i) {
  var r, o, a, u;
  return !!((u = (a = (o = (r = n == null ? void 0 : n[e]) == null ? void 0 : r[t]) == null ? void 0 : o.dataTypes) == null ? void 0 : a.includes) != null && u.call(a, i));
}
function Io(n, e) {
  if (e === "fp16" && n.desc.dataType === "Float16")
    return new Uint8Array(
      n.data.buffer,
      n.data.byteOffset,
      n.data.byteLength
    );
  if (e === "fp32" && n.desc.dataType === "Float32")
    return new Uint8Array(
      n.data.buffer,
      n.data.byteOffset,
      n.data.byteLength
    );
  const t = n.desc.dataType === "Float32" ? new Float32Array(
    n.data.buffer,
    n.data.byteOffset,
    n.data.byteLength / 4
  ) : new E(
    n.data.buffer,
    n.data.byteOffset,
    n.data.byteLength / 2
  ), i = e === "fp16" ? new E(t) : new Float32Array(t);
  return new Uint8Array(
    i.buffer,
    i.byteOffset,
    i.byteLength
  );
}
function Bo(n) {
  const e = Array.from(
    { length: n },
    (i, r) => `@group(0) @binding(${r}) var<storage, read> input${r}: array<vec4<f32>>;`
  ).join(`
`), t = Array.from({ length: n }, (i, r) => {
    const o = r * 3;
    return `if (channel < ${o + 3}u) {
      return input${r}[pixel][channel - ${o}u];
    }`;
  }).join(`
  `);
  return (
    /* wgsl */
    `enable f16;
struct Params { width: u32, height: u32, channels: u32, padding: u32 }
${e}
@group(0) @binding(${n}) var<storage, read_write> outputData: array<f16>;
@group(0) @binding(${n + 1}) var<uniform> params: Params;

fn readChannel(pixel: u32, channel: u32) -> f32 {
  ${t}
  return 0.0;
}

@compute @workgroup_size(${pe}, ${pe}, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= params.width || gid.y >= params.height || gid.z >= params.channels) {
    return;
  }
  let pixel = gid.y * params.width + gid.x;
  let outputIndex = (gid.z * params.height + gid.y) * params.width + gid.x;
  outputData[outputIndex] = f16(readChannel(pixel, gid.z));
}
`
  );
}
function To() {
  return (
    /* wgsl */
    `enable f16;
struct Params { width: u32, height: u32, padding0: u32, padding1: u32 }
@group(0) @binding(0) var<storage, read> inputData: array<f16>;
@group(0) @binding(1) var<storage, read_write> outputData: array<vec4<f32>>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(${pe}, ${pe}, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= params.width || gid.y >= params.height) { return; }
  let pixel = gid.y * params.width + gid.x;
  let plane = params.width * params.height;
  outputData[pixel] = vec4<f32>(
    f32(inputData[pixel]),
    f32(inputData[plane + pixel]),
    f32(inputData[plane * 2u + pixel]),
    0.0
  );
}
`
  );
}
function Po(n, e) {
  if (e === "fp32")
    throw new Error(
      "OIDN WebNN GPU interop currently requires FP16 exportable tensors"
    );
  if (!n.features.has("shader-f16"))
    throw new Error("OIDN WebNN requires shader-f16 on the shared GPUDevice");
  return "fp16";
}
function Co(n, e, t) {
  return t === "relu" ? n.relu(e) : e;
}
class So {
  constructor(e, t, i = {}) {
    m(this, "precision");
    m(this, "support");
    m(this, "_context");
    m(this, "_builderConstructor");
    m(this, "_shapeCache", /* @__PURE__ */ new Map());
    m(this, "_shapePromises", /* @__PURE__ */ new Map());
    m(this, "_retiredExecutions", /* @__PURE__ */ new Set());
    m(this, "_pendingCreationCount", 0);
    m(this, "_shapeCacheSize");
    m(this, "_clock", 0);
    m(this, "_inputPipeline");
    m(this, "_outputPipeline");
    m(this, "_resources", new cn());
    m(this, "_disposed", !1);
    this._device = e, this._model = t, this.precision = Po(
      e,
      i.precision ?? "auto"
    ), this._shapeCacheSize = Math.max(1, i.shapeCacheSize ?? 2), this.support = {
      available: !1,
      fp16Conv: !1,
      gpuInterop: !1
    };
    const r = bo(
      e,
      t.inputChannels / 3
    );
    this._inputPipeline = r.input, this._outputPipeline = r.output;
  }
  async prepare() {
    var i, r, o;
    if (this._disposed) throw new Error("OIDN WebNN executor is disposed");
    const e = (i = globalThis.navigator) == null ? void 0 : i.ml, t = globalThis.MLGraphBuilder;
    if (!(e != null && e.createContext) || typeof t != "function")
      throw this.support.reason = "WebNN is not exposed by this browser", new Error(this.support.reason);
    this._builderConstructor = t;
    try {
      try {
        this._context = await e.createContext({
          deviceType: "gpu",
          powerPreference: "high-performance"
        });
      } catch {
        this._context = await e.createContext({ deviceType: "gpu" });
      }
      if (this._resources.track("ml-context", this._context), this._disposed) throw new Error("OIDN WebNN executor is disposed");
      if (typeof this._context.createExportableTensor != "function" || typeof this._context.exportToGPU != "function")
        throw this.support.reason = "WebNN WebGPU tensor interop is unavailable", new Error(this.support.reason);
      const a = ((o = (r = this._context).opSupportLimits) == null ? void 0 : o.call(r)) ?? {};
      if (this.support.fp16Conv = gt(a, "conv2d", "input", "float16") && gt(a, "conv2d", "filter", "float16") && gt(a, "conv2d", "output", "float16"), !this.support.fp16Conv)
        throw this.support.reason = "WebNN does not support FP16 conv2d", new Error(this.support.reason);
      let u, s;
      try {
        u = this._resources.track(
          "ml-tensor",
          await this._context.createExportableTensor(
            { dataType: "float16", shape: [4] },
            this._device
          )
        ), s = this._resources.track(
          "gpu-buffer",
          await this._context.exportToGPU(u)
        ), this.support.gpuInterop = !0;
      } catch (l) {
        throw this.support.reason = `WebNN FP16 WebGPU interop failed: ${String(l)}`, new Error(this.support.reason);
      } finally {
        this._releaseBuffer(s), this._releaseTensor(u);
      }
      this.support.available = !0;
    } catch (a) {
      throw this._releaseContext(), a;
    }
  }
  _constant(e, t) {
    return e.constant(
      {
        dataType: "float16",
        shape: [...t.desc.dims]
      },
      Io(t, this.precision)
    );
  }
  async _createExecution(e, t) {
    if (this._disposed) throw new Error("OIDN WebNN executor is disposed");
    const i = new this._builderConstructor(this._context), r = /* @__PURE__ */ new Map(), o = /* @__PURE__ */ new Map();
    r.set(
      this._model.spec.input,
      i.input("input", {
        dataType: "float16",
        shape: [1, this._model.inputChannels, t, e]
      })
    ), o.set(this._model.spec.input, [this._model.inputChannels, t, e]);
    for (const d of this._model.spec.nodes) {
      let h, f;
      if (d.op === "conv2d") {
        const g = o.get(d.input), y = this._model.convTensors.get(d.id), v = i.conv2d(
          r.get(d.input),
          this._constant(i, y.weight),
          {
            bias: this._constant(i, y.bias),
            padding: [1, 1, 1, 1],
            inputLayout: "nchw",
            filterLayout: "oihw"
          }
        );
        h = Co(i, v, d.activation), f = [y.outputChannels, g[1], g[2]];
      } else if (d.op === "maxPool2d") {
        const g = o.get(d.input);
        h = i.maxPool2d(r.get(d.input), {
          windowDimensions: [2, 2],
          strides: [2, 2],
          padding: [0, g[1] % 2, 0, g[2] % 2],
          layout: "nchw"
        }), f = [
          g[0],
          Math.ceil(g[1] / 2),
          Math.ceil(g[2] / 2)
        ];
      } else if (d.op === "upsample2d") {
        const g = o.get(d.input);
        h = i.resample2d(r.get(d.input), {
          mode: "nearest-neighbor",
          axes: [2, 3],
          scales: [2, 2]
        }), f = [g[0], g[1] * 2, g[2] * 2];
      } else {
        const g = d.inputs.map((y) => o.get(y));
        if (g.some(
          (y) => y[1] !== g[0][1] || y[2] !== g[0][2]
        ))
          throw new Error(
            `WebNN concat ${d.id} has mismatched spatial shapes`
          );
        h = i.concat(
          d.inputs.map((y) => r.get(y)),
          1
        ), f = [
          g.reduce((y, v) => y + v[0], 0),
          g[0][1],
          g[0][2]
        ];
      }
      r.set(d.id, h), o.set(d.id, f);
    }
    let a, u, s, l, p, c;
    try {
      return a = this._resources.track(
        "ml-graph",
        await i.build({
          output: r.get(this._model.spec.output)
        })
      ), u = this._resources.track(
        "ml-tensor",
        await this._context.createExportableTensor(
          {
            dataType: "float16",
            shape: [1, this._model.inputChannels, t, e],
            writable: !0
          },
          this._device
        )
      ), s = this._resources.track(
        "ml-tensor",
        await this._context.createExportableTensor(
          {
            dataType: "float16",
            shape: [1, this._model.outputChannels, t, e],
            readable: !0
          },
          this._device
        )
      ), l = this._resources.track(
        "gpu-buffer",
        this._device.createBuffer({
          label: `oidn/webnn/output/${e}x${t}`,
          size: e * t * 4 * 4,
          usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC | GPUBufferUsage.COPY_DST
        })
      ), p = this._resources.track(
        "gpu-buffer",
        di(
          this._device,
          `oidn/webnn/input/${e}x${t}`,
          [e, t, this._model.inputChannels]
        )
      ), c = this._resources.track(
        "gpu-buffer",
        di(
          this._device,
          `oidn/webnn/output/${e}x${t}`,
          [e, t]
        )
      ), {
        graph: a,
        inputTensor: u,
        outputTensor: s,
        outputBuffer: l,
        inputUniform: p,
        outputUniform: c,
        width: e,
        height: t,
        lastUsed: ++this._clock
      };
    } catch (d) {
      throw this._releaseBuffer(c), this._releaseBuffer(p), this._releaseBuffer(l), this._releaseTensor(s), this._releaseTensor(u), this._releaseGraph(a), d;
    }
  }
  async _execution(e, t) {
    const i = `${e}x${t}`;
    let r = this._shapeCache.get(i);
    if (!r) {
      let o = this._shapePromises.get(i);
      o || (o = (async () => {
        this._pendingCreationCount++;
        try {
          return await this._createExecution(e, t);
        } finally {
          this._pendingCreationCount--;
        }
      })(), this._shapePromises.set(i, o));
      try {
        if (r = await o, this._disposed)
          throw this._destroyExecution(r), new Error("OIDN WebNN executor is disposed");
        this._shapeCache.set(i, r);
      } finally {
        this._shapePromises.get(i) === o && this._shapePromises.delete(i);
      }
      if (this._shapeCache.size > this._shapeCacheSize) {
        const a = [...this._shapeCache.entries()].filter(([u]) => u !== i).sort((u, s) => u[1].lastUsed - s[1].lastUsed)[0];
        a && (this._shapeCache.delete(a[0]), this._retireExecution(a[1]));
      }
    }
    return r.lastUsed = ++this._clock, r;
  }
  /** Compiles common tile shapes while the host still reports model loading. */
  async prewarm(e) {
    for (const t of e)
      await this._execution(t.width, t.height);
  }
  async execute(e, t, i) {
    const r = this._model.inputChannels / 3;
    if (e.length !== r)
      throw new Error(
        `OIDN WebNN expected ${r} input buffers, got ${e.length}`
      );
    const o = await this._execution(t, i), a = this._resources.track(
      "gpu-buffer",
      await this._context.exportToGPU(o.inputTensor)
    );
    try {
      const s = e.map(
        (d, h) => ({ binding: h, resource: { buffer: d } })
      );
      s.push({
        binding: r,
        resource: { buffer: a }
      }), s.push({
        binding: r + 1,
        resource: { buffer: o.inputUniform }
      });
      const l = this._device.createBindGroup({
        label: "oidn/webnn/input-bindings",
        layout: this._inputPipeline.getBindGroupLayout(0),
        entries: s
      }), p = this._device.createCommandEncoder({
        label: "oidn/webnn/input-pack"
      }), c = p.beginComputePass();
      c.setPipeline(this._inputPipeline), c.setBindGroup(0, l), c.dispatchWorkgroups(
        Math.ceil(t / pe),
        Math.ceil(i / pe),
        this._model.inputChannels
      ), c.end(), this._device.queue.submit([p.finish()]);
    } finally {
      this._releaseBuffer(a);
    }
    this._context.dispatch(
      o.graph,
      { input: o.inputTensor },
      { output: o.outputTensor }
    );
    const u = this._resources.track(
      "gpu-buffer",
      await this._context.exportToGPU(o.outputTensor)
    );
    try {
      const s = this._device.createBindGroup({
        label: "oidn/webnn/output-bindings",
        layout: this._outputPipeline.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: { buffer: u } },
          { binding: 1, resource: { buffer: o.outputBuffer } },
          { binding: 2, resource: { buffer: o.outputUniform } }
        ]
      }), l = this._device.createCommandEncoder({
        label: "oidn/webnn/output-unpack"
      }), p = l.beginComputePass();
      p.setPipeline(this._outputPipeline), p.setBindGroup(0, s), p.dispatchWorkgroups(
        Math.ceil(t / pe),
        Math.ceil(i / pe)
      ), p.end(), this._device.queue.submit([l.finish()]);
    } finally {
      this._releaseBuffer(u);
    }
    return o.outputBuffer;
  }
  async executeCPU(e, t, i) {
    const r = await this._execution(t, i), o = t * i, a = new E(o * this._model.inputChannels);
    for (let l = 0; l < o; l++)
      for (let p = 0; p < this._model.inputChannels; p++)
        a[p * o + l] = e[l * this._model.inputChannels + p];
    this._context.writeTensor(r.inputTensor, a), this._context.dispatch(
      r.graph,
      { input: r.inputTensor },
      { output: r.outputTensor }
    );
    const u = new E(
      await this._context.readTensor(r.outputTensor)
    ), s = new Float32Array(o * this._model.outputChannels);
    for (let l = 0; l < o; l++)
      for (let p = 0; p < this._model.outputChannels; p++)
        s[l * this._model.outputChannels + p] = u[p * o + l];
    return s;
  }
  _destroyExecution(e) {
    this._releaseGraph(e.graph), this._releaseTensor(e.inputTensor), this._releaseTensor(e.outputTensor), this._releaseBuffer(e.outputBuffer), this._releaseBuffer(e.inputUniform), this._releaseBuffer(e.outputUniform);
  }
  _retireExecution(e) {
    this._retiredExecutions.add(e), this._device.queue.onSubmittedWorkDone().catch(() => {
    }).then(() => {
      this._retiredExecutions.delete(e), this._destroyExecution(e);
    });
  }
  _releaseBuffer(e) {
    this._resources.release("gpu-buffer", e, () => e.destroy());
  }
  _releaseTensor(e) {
    this._resources.release("ml-tensor", e, () => e.destroy());
  }
  _releaseGraph(e) {
    this._resources.release("ml-graph", e, () => {
      var t;
      return (t = e.destroy) == null ? void 0 : t.call(e);
    });
  }
  _releaseContext() {
    this._resources.release(
      "ml-context",
      this._context,
      () => {
        var e, t;
        return (t = (e = this._context).destroy) == null ? void 0 : t.call(e);
      }
    );
  }
  getResourceInfo() {
    return this._resources.snapshot(
      this._pendingCreationCount + this._retiredExecutions.size
    );
  }
  dispose() {
    if (!this._disposed) {
      this._disposed = !0;
      for (const e of this._shapeCache.values())
        this._destroyExecution(e);
      this._shapeCache.clear();
      for (const e of this._retiredExecutions)
        this._destroyExecution(e);
      this._retiredExecutions.clear(), this._shapePromises.clear(), this._releaseContext();
    }
  }
}
const Eo = 100;
function fe(n, e) {
  return Math.ceil(n / e) * e;
}
function Ge(n) {
  return n.data instanceof GPUBuffer || n.data instanceof GPUTexture;
}
class Ao {
  constructor(e, t, i = {}) {
    m(this, "_device");
    m(this, "_aux");
    m(this, "_hdr");
    m(this, "_hdrTransfer");
    m(this, "_dataProcessGPU");
    m(this, "_nativeExecutor");
    m(this, "_webNNExecutor");
    m(this, "_modelSpec");
    m(this, "_inputChannels");
    m(this, "_engine");
    m(this, "_dynamicTileController");
    m(this, "_lastExecution");
    m(this, "_activeExecutionFailures", /* @__PURE__ */ new Set());
    m(this, "_deviceLostObserved", !1);
    m(this, "_deviceLostSettled", !1);
    m(this, "_deviceLostReason");
    this._aux = i.aux || !1, this._hdr = i.hdr || !1, this._hdrTransfer = i.hdrTransfer ?? "pu", this._engine = i.engine ?? "auto";
    const r = i.modelSpec ?? Li(e), o = rr(e, r);
    this._modelSpec = o.spec, this._inputChannels = o.inputChannels;
    const a = this._aux ? 9 : 3;
    if (o.inputChannels !== a)
      throw new Error(
        `OIDN model expects ${o.inputChannels} input channels, but aux=${this._aux} provides ${a}`
      );
    this._dynamicTileController = new er(
      i.maxTileSize ?? 512,
      i.dynamicTile
    ), this._device = t, this._observeDeviceLoss(), this._engine === "webnn" ? this._webNNExecutor = new So(
      this._device,
      o,
      { precision: i.precision }
    ) : this._nativeExecutor = new $o(
      this._device,
      o,
      { precision: i.precision, kernel: i.kernel, gemm: i.gemm }
    );
  }
  getDevice() {
    return this._device;
  }
  /** Completes backend compilation before first interactive use. */
  async prepare() {
    if (this._webNNExecutor) {
      await this._webNNExecutor.prepare();
      const e = fe(
        this._modelSpec.receptiveField / 2,
        W
      ), t = [
        this._dynamicTileController.tileSize,
        this._dynamicTileController.minTileSize
      ];
      await this._webNNExecutor.prewarm(
        [...new Set(t)].map((i) => ({
          width: i + 2 * e,
          height: i + 2 * e
        }))
      );
      return;
    }
    await this._nativeExecutor.prepare();
  }
  /**
   * Prepares the input shapes selected for an image before its first denoise.
   * Hosts can call this while they still display their model-loading state.
   */
  async prepareForImage(e, t, i = {}) {
    var s, l;
    const r = fe(
      this._modelSpec.receptiveField / 2,
      W
    ), o = i.tileOverlap === void 0 ? r : fe(Math.max(0, i.tileOverlap), W), a = Dt(
      e,
      t,
      i.wholeImage ? fe(Math.max(e, t), W) : this._dynamicTileController.tileSize,
      o
    ), u = [...new Map(
      a.tiles.map(({ input: p }) => [
        `${p.width}x${p.height}`,
        { width: p.width, height: p.height }
      ])
    ).values()];
    await ((s = this._webNNExecutor) == null ? void 0 : s.prewarm(u)), (l = this._nativeExecutor) == null || l.prewarm(u);
  }
  getRuntimeInfo() {
    var e;
    return {
      configuredEngine: this._engine,
      gpuEngine: this._webNNExecutor ? "webnn" : "wgsl",
      precision: (this._webNNExecutor ?? this._nativeExecutor).precision,
      kernel: this._nativeExecutor ? {
        configured: this._nativeExecutor.kernelSetting,
        gemm: this._nativeExecutor.gemm,
        maxSpatialInputBlocks: this._nativeExecutor.maxSpatialInputBlocks,
        subgroupsAvailable: this._nativeExecutor.subgroupsAvailable
      } : void 0,
      webnn: (e = this._webNNExecutor) == null ? void 0 : e.support,
      resources: (this._webNNExecutor ?? this._nativeExecutor).getResourceInfo(),
      model: this._modelSpec.id,
      modelFamily: this._modelSpec.family,
      inputChannels: this._inputChannels,
      hdrTransfer: this._hdrTransfer,
      dynamicTile: {
        enabled: this._dynamicTileController.enabled,
        currentTileSize: this._dynamicTileController.tileSize,
        minTileSize: this._dynamicTileController.minTileSize,
        maxTileSize: this._dynamicTileController.maxTileSize,
        targetTileTimeMs: this._dynamicTileController.targetTileTimeMs
      },
      lastExecution: this._lastExecution,
      activeExecutionCount: this._activeExecutionFailures.size
    };
  }
  _observeDeviceLoss() {
    if (this._activeExecutionFailures ?? (this._activeExecutionFailures = /* @__PURE__ */ new Set()), this._deviceLostObserved) return;
    this._deviceLostObserved = !0;
    const e = this._device.lost;
    if (!e) return;
    const t = (i) => {
      if (this._deviceLostSettled) return;
      this._deviceLostSettled = !0, this._deviceLostReason = i;
      const r = [...this._activeExecutionFailures];
      this._activeExecutionFailures.clear();
      for (const o of r) o(i);
    };
    e.then(
      (i) => t(new Error(`WebGPU device lost: ${i.message}`)),
      t
    );
  }
  _registerExecutionFailure(e) {
    return this._observeDeviceLoss(), this._activeExecutionFailures.add(e), this._deviceLostSettled && (this._activeExecutionFailures.delete(e), queueMicrotask(() => e(this._deviceLostReason))), () => this._activeExecutionFailures.delete(e);
  }
  /** Captures per-node GPU timestamps for the next native tile execution. */
  profileNextExecution() {
    var e;
    return ((e = this._nativeExecutor) == null ? void 0 : e.profileNextExecution()) ?? !1;
  }
  getLastExecutionProfile() {
    var e;
    return (e = this._nativeExecutor) == null ? void 0 : e.getLastExecutionProfile();
  }
  _processImageData(e, t, i, r) {
    const o = e.data, a = o.length / 4, u = this._aux ? 9 : 3, s = new Float32Array(a * u);
    if (t && !i || i && !t)
      throw new Error("Normal map and albedo map are both required");
    if (t && i && (t.width !== i.width || t.height !== i.height || e.width !== t.width || e.height !== t.height))
      throw new Error("Image size mismatch");
    const l = t == null ? void 0 : t.data, p = i == null ? void 0 : i.data;
    for (let c = 0; c < o.length; c += 4) {
      const d = c / 4 * u;
      for (let h = 0; h < 3; h++)
        r ? s[d + h] = o[c + h] : s[d + h] = o[c + h] / 255, l && (s[d + h + 3] = l[c + h] / 255), p && (s[d + h + 6] = p[c + h] / 255);
    }
    return s;
  }
  _readTile(e, t, i, r) {
    const o = new Float32Array(
      i.width * i.height * t
    ), a = e.length / (r * t);
    for (let u = 0; u < i.height; u++)
      for (let s = 0; s < i.width; s++) {
        const l = Math.min(r - 1, s + i.x), c = (Math.min(a - 1, u + i.y) * r + l) * t, d = (u * i.width + s) * t;
        for (let h = 0; h < t; h++)
          o[d + h] = e[c + h];
      }
    return o;
  }
  _writeTile(e, t, i, r, o, a) {
    const { data: u, width: s } = e, l = i.x - t.x, p = i.y - t.y;
    for (let c = 0; c < i.height; c++)
      for (let d = 0; d < i.width; d++) {
        const h = ((c + p) * o + d + l) * 3, f = ((c + i.y) * s + (d + i.x)) * 4;
        for (let g = 0; g < 3; g++)
          a ? u[f + g] = r[h + g] : u[f + g] = Math.min(
            Math.max(r[h + g] * 255, 0),
            255
          );
        e.data[f + 3] = a ? 1 : 255;
      }
  }
  async _executeTile(e, t, i, r, o, a, u, s, l) {
    const p = this._aux ? 9 : 3, c = new ct(
      r.input.x,
      r.input.y,
      r.input.width,
      r.input.height
    ), d = new ct(
      r.output.x,
      r.output.y,
      r.output.width,
      r.output.height
    ), h = c.width, f = c.height;
    let g, y, v = 1;
    const _ = this._device;
    let w = this._dataProcessGPU;
    if (e instanceof Float32Array) {
      let x = this._readTile(e, p, c, a);
      s && (v = Hn({
        data: x,
        channels: p
      }), x = Wn({
        data: x,
        channels: p,
        inputScale: v,
        transfer: this._hdrTransfer
      })), y = await (this._webNNExecutor ?? this._nativeExecutor).executeCPU(
        x,
        h,
        f
      );
    } else {
      w || (w = this._dataProcessGPU = new jn(
        _,
        s,
        this._hdrTransfer
      )), w.setImageSize(a, u), w.setInputTile(c), o && w.copyInputDataToOutput(e.color);
      const { color: x, albedo: B, normal: k } = w.forward(
        e.color,
        this._aux ? e.albedo : void 0,
        this._aux ? e.normal : void 0,
        l
      );
      g = await (this._webNNExecutor ?? this._nativeExecutor).execute(
        this._aux ? [x, B, k] : [x],
        h,
        f
      );
    }
    let $;
    if (e instanceof Float32Array) {
      s && (y = Dn({
        data: y,
        channels: 3,
        inputScale: v,
        transfer: this._hdrTransfer
      })), this._writeTile(
        i,
        c,
        d,
        y,
        c.width,
        s
      );
      for (let x = 0; x < d.height; x++)
        for (let B = 0; B < d.width; B++) {
          const k = (x * d.width + B) * 4, C = ((x + d.y) * a + (B + d.x)) * 4;
          for (let z = 0; z < 4; z++)
            t.data[k + z] = i.data[C + z];
        }
    } else
      w.setOutputTile(d, c), $ = w.inverse(
        g,
        e.color
      );
    return $;
  }
  tileExecute({
    color: e,
    albedo: t,
    normal: i,
    done: r,
    progress: o,
    denoiseAlpha: a,
    tileOverlap: u,
    wholeImage: s,
    scheduling: l = "event-loop",
    error: p
  }) {
    if (this._aux && (!t || !i))
      throw new Error("Normal map and albedo map are both required");
    if (!this._aux && (t || i))
      throw new Error("Normal map and albedo map are not required");
    const c = e.width, d = e.height, h = this._dynamicTileController.tileSize, f = s ? fe(Math.max(c, d), W) : h, g = fe(
      this._modelSpec.receptiveField / 2,
      W
    ), y = u === void 0 ? g : fe(Math.max(0, u), W), v = Dt(
      c,
      d,
      f,
      y
    ), _ = v.tiles.length > 1, w = this._hdr || !1;
    let $;
    Ge(e) || ($ = this._processImageData(
      e,
      t,
      i,
      w
    ));
    function x(F, V) {
      return w ? {
        data: new Float32Array(F * V * 4),
        width: F,
        height: V
      } : new ImageData(F, V);
    }
    const B = Ge(e) ? void 0 : x(c, d);
    let k = "active", C, z, X = () => !1;
    const N = () => typeof performance > "u" ? Date.now() : performance.now(), ye = N(), R = [], j = () => {
      C !== void 0 && (clearTimeout(C), C = void 0), z !== void 0 && typeof cancelAnimationFrame < "u" && (cancelAnimationFrame(z), z = void 0);
    }, Be = (F) => {
      console.error("OIDN error callback failed", F);
    }, ot = (F) => {
      if (k === "active")
        if (k = "settled", j(), X(), p)
          try {
            Promise.resolve(p(F)).catch(Be);
          } catch (V) {
            Be(V);
          }
        else
          console.error("OIDN execution failed", F);
    }, _n = (F) => {
      if (l === "event-loop" || typeof requestAnimationFrame > "u")
        C = setTimeout(() => {
          C = void 0, F();
        }, 0);
      else {
        const V = () => {
          j(), F();
        };
        z = requestAnimationFrame(V), C = setTimeout(V, Eo);
      }
    }, Nt = async (F) => {
      if (k !== "active")
        return;
      const V = v.tiles[F], Ut = Ge(e) ? void 0 : x(V.output.width, V.output.height), yn = N(), wn = await this._executeTile(
        Ge(e) ? {
          color: e.data,
          albedo: t == null ? void 0 : t.data,
          normal: i == null ? void 0 : i.data
        } : $,
        Ut,
        B,
        V,
        F === 0,
        c,
        d,
        w,
        a
      );
      if (k !== "active") return;
      const zt = B || {
        data: wn,
        width: c,
        height: d
      };
      if (o && await o(
        zt,
        // Is undefined if using webgpu buffer
        Ut,
        new ct(
          V.output.x,
          V.output.y,
          V.output.width,
          V.output.height
        ),
        F,
        v.tiles.length
      ), k !== "active") return;
      const vn = F + 1 < v.tiles.length;
      if (await this._device.queue.onSubmittedWorkDone(), k !== "active") return;
      await (async () => {
        if (R.push(N() - yn), k === "active")
          if (vn)
            _n(() => {
              k === "active" && Nt(F + 1).catch(ot);
            });
          else {
            const Q = [...R].sort((st, ut) => st - ut), at = Math.floor(Q.length / 2), xn = Q.length % 2 ? Q[at] : (Q[at - 1] + Q[at]) / 2;
            this._lastExecution = {
              width: c,
              height: d,
              tileCount: v.tiles.length,
              tileColumns: v.columns,
              tileRows: v.rows,
              tileOverlap: v.overlap,
              inputPixelCount: v.inputPixelCount,
              inputShapeCount: v.inputShapeCount,
              durationMs: N() - ye,
              tileTimeMs: {
                min: Q[0],
                median: xn,
                mean: Q.reduce((st, ut) => st + ut, 0) / Q.length,
                max: Q[Q.length - 1]
              }
            }, _ && this._dynamicTileController.observe(R), X(), await r(zt), k === "active" && (k = "settled");
          }
      })();
    };
    return X = this._registerExecutionFailure(ot), Nt(0).catch(ot), () => {
      k === "active" && (k = "aborted", j(), X());
    };
  }
  dispose() {
    var e, t, i;
    (e = this._dataProcessGPU) == null || e.dispose(), (t = this._nativeExecutor) == null || t.dispose(), (i = this._webNNExecutor) == null || i.dispose();
  }
}
async function Mo() {
  if (!navigator.gpu) throw new Error("WebGPU is not available");
  const n = {
    powerPreference: "high-performance"
  }, e = await navigator.gpu.requestAdapter(n);
  if (!e) throw new Error("No WebGPU adapter is available");
  const t = {}, i = [];
  e.features.has("timestamp-query") && i.push("timestamp-query"), e.features.has("bgra8unorm-storage") && i.push("bgra8unorm-storage"), e.features.has("shader-f16") && i.push("shader-f16"), t.requiredFeatures = i;
  const r = e.limits;
  return t.requiredLimits = {
    maxComputeWorkgroupStorageSize: r.maxComputeWorkgroupStorageSize,
    maxComputeWorkgroupsPerDimension: r.maxComputeWorkgroupsPerDimension,
    maxStorageBufferBindingSize: r.maxStorageBufferBindingSize,
    maxBufferSize: r.maxBufferSize,
    maxComputeWorkgroupSizeX: r.maxComputeWorkgroupSizeX,
    maxComputeInvocationsPerWorkgroup: r.maxComputeInvocationsPerWorkgroup
  }, e.requestDevice(t);
}
async function Oo(n, e, t) {
  const i = (e == null ? void 0 : e.device) ?? await Mo(), r = Tn(n), o = new Ao(r, i, t);
  return await o.prepare(), o;
}
async function zo(n, e, t) {
  return fetch(n).then((i) => i.arrayBuffer()).then((i) => Oo(i, e, t));
}
export {
  $o as NativeUNetExecutor,
  ir as OIDN_UNET_LARGE_SPEC,
  tr as OIDN_UNET_SMALL_SPEC,
  Ao as UNet,
  So as WebNNUNetExecutor,
  Li as detectUNetModelSpec,
  Oo as initUNetFromBuffer,
  zo as initUNetFromURL,
  kt as optimizeModelGraph,
  Tn as parseTZA,
  ro as planModelExecution,
  Dt as planTileGrid,
  xo as resolveNativeUNetPrecision,
  rr as validateUNetModel
};
