// The lamp's fittings (fixture.js) as three.js meshes: holder, the cap's metal, the frosted
// bulb and the cable, in the fittings' own frame (z up from the socket face), placed where the
// recipe has the socket. Shared by the 3D view and the render.

import * as THREE from 'three';
import { CABLE_D, CAP_SHOWS, bulbProfile, capThread, capTop, socketProfile, socketThread, threadRadius } from './fixture.js';

/** A lathe round the z axis from [radius, z] points. */
function lathe(profile, segments = 64) {
  const g = new THREE.LatheGeometry(profile.map(([r, z]) => new THREE.Vector2(Math.max(0, r), z)), segments);
  g.rotateX(Math.PI / 2);
  g.computeVertexNormals();
  return g;
}

/**
 * The socket's thread as a surface: round the body, a row of points every few degrees, each row
 * up its height, the radius `threadRadius` (a tooth a pitch, rising a pitch a turn). Closed round
 * (no seam), open at its ends, where it meets the body at the root.
 * @param {NonNullable<ReturnType<typeof socketThread>>} t
 */
function threadGeometry(t, around = 144, perPitch = 12) {
  const rows = Math.max(2, Math.ceil(((t.z1 - t.z0) / t.pitch) * perPitch));
  const pos = new Float32Array(around * (rows + 1) * 3);
  for (let j = 0; j <= rows; j++) {
    const z = t.z0 + ((t.z1 - t.z0) * j) / rows;
    for (let i = 0; i < around; i++) {
      const a = (2 * Math.PI * i) / around;
      const r = threadRadius(t, a, z);
      const k = 3 * (j * around + i);
      pos[k] = r * Math.cos(a);
      pos[k + 1] = r * Math.sin(a);
      pos[k + 2] = z;
    }
  }
  const index = [];
  for (let j = 0; j < rows; j++) {
    for (let i = 0; i < around; i++) {
      const [a, b] = [j * around + i, j * around + ((i + 1) % around)];
      const [c, d] = [a + around, b + around];
      index.push(a, b, d, a, d, c);
    }
  }
  const g = new THREE.BufferGeometry();
  g.setAttribute('position', new THREE.BufferAttribute(pos, 3));
  g.setIndex(index);
  g.computeVertexNormals();
  return g;
}

/**
 * The fittings, placed in the recipe's frame (the socket face at `fx.face`, the body rising
 * along `fx.up`). Each mesh is named: 'socket' (its body), 'socket-thread' (the thread the shade
 * screws onto, at its pitch), 'cap', 'bulb', 'cable'.
 * @param {NonNullable<ReturnType<typeof import('./fixture.js').fixtureOf>>} fx
 * @param {{ socket: THREE.Material, cap: THREE.Material, bulb: THREE.Material, cable: THREE.Material }} materials
 * @param {number} [cable] the cable's length above the gland (mm)
 */
export function fixtureGroup(fx, materials, cable = 500) {
  const group = new THREE.Group();
  const add = (name, geometry, material) => {
    const m = new THREE.Mesh(geometry, material);
    m.name = name;
    group.add(m);
    return m;
  };
  add('socket', lathe(socketProfile(fx)), materials.socket);
  const thread = socketThread(fx);
  if (thread) add('socket-thread', threadGeometry(thread), materials.socket);
  // The bulb (and its cap) unscrewed from the socket when taken apart: this far below its face.
  const drop = /** @type {any} */ (fx).bulbDrop || 0;
  // The Edison screw cap: its core at the thread's root, the thread over it, then the black
  // insulator and the contact. Screwed in, all but its bottom is inside the holder.
  const ct = capThread(fx);
  const down = (profile) => profile.map(([r, z]) => [r, z - drop]);
  add('cap', lathe(down([[0, ct.z0], [ct.root, ct.z0], [ct.root, ct.z1], [0, ct.z1]]), 48), materials.cap);
  const capScrew = threadGeometry(ct, 96, 10);
  capScrew.translate(0, 0, -drop);
  add('cap-thread', capScrew, materials.cap);
  const capEnd = capTop(fx);
  add('cap-insulator', lathe(down(capEnd.insulator), 32), materials.socket);
  add('cap-contact', lathe(down(capEnd.contact), 24), materials.cap);
  add('bulb', lathe(bulbProfile(fx).map(([r, d]) => [r, -(drop + CAP_SHOWS + d)]), 96), materials.bulb);
  const top = fx.height + fx.glandH;
  const wire = new THREE.CylinderGeometry(CABLE_D / 2, CABLE_D / 2, cable, 24, 1, false);
  wire.rotateX(Math.PI / 2);
  wire.translate(0, 0, top + cable / 2);
  add('cable', wire, materials.cable);
  group.position.set(0, 0, fx.face);
  group.rotation.set(fx.up < 0 ? Math.PI : 0, 0, 0);
  return group;
}
