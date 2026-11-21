// The lamp's fittings (fixture.js) as three.js meshes: holder, the cap's metal, the frosted
// bulb and the cable, in the fittings' own frame (z up from the socket face), placed where the
// recipe has the socket. Shared by the 3D view and the render.

import * as THREE from 'three';
import { CABLE_D, CAP_SHOWS, bulbProfile, socketProfile } from './fixture.js';

/** A lathe round the z axis from [radius, z] points. */
function lathe(profile, segments = 64) {
  const g = new THREE.LatheGeometry(profile.map(([r, z]) => new THREE.Vector2(Math.max(0, r), z)), segments);
  g.rotateX(Math.PI / 2);
  g.computeVertexNormals();
  return g;
}

/**
 * The fittings, placed in the recipe's frame (the socket face at `fx.face`, the body rising
 * along `fx.up`). Each mesh is named: 'socket', 'cap', 'bulb', 'cable'.
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
  const neck = fx.cap / 2;
  add('cap', lathe([[0, 0], [neck, 0], [neck, -CAP_SHOWS], [0, -CAP_SHOWS]], 48), materials.cap);
  add('bulb', lathe(bulbProfile(fx).map(([r, d]) => [r, -(CAP_SHOWS + d)]), 96), materials.bulb);
  const top = fx.height + fx.glandH;
  const wire = new THREE.CylinderGeometry(CABLE_D / 2, CABLE_D / 2, cable, 24, 1, false);
  wire.rotateX(Math.PI / 2);
  wire.translate(0, 0, top + cable / 2);
  add('cable', wire, materials.cable);
  group.position.set(0, 0, fx.face);
  group.rotation.set(fx.up < 0 ? Math.PI : 0, 0, 0);
  return group;
}
