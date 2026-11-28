# The Render view: the lamp's light, path traced

How a lampshade looks with its bulb on (and off) depends on how the light bounces off it,
passes through it and bends through its beads. The **Render** view (next to Shape, Toolpath,
Both, Plan) works that out with a path tracer and saves a PNG with a transparent background,
to place on top of an environment afterwards.

## The fittings: socket, bulb, cable

Every recipe with a `socket` setting (Lampshade, Lamp, Thread pendant) shows a standard
lampholder where the shade screws on, a frosted bulb in it and a cable going straight up, in
the 3D view, the Plan and the render (`web/js/fixture.js`, its meshes `fixture-mesh.js`).
In the Plan, the bulb button under the zoom buttons hides and shows them (remembered on the
device).

| Socket | Holder (body Ø × height) | Thread pitch | Bulb | Glass Ø | Glass below the cap |
|---|---|---|---|---|---|
| E27 | 40 × 55 mm | 2.5 mm | A60 (pear) | 60 mm | 83 mm (about 108 mm with the cap) |
| E14 | 28 × 45 mm | 2 mm | P45 (round) | 45 mm | 58 mm (about 80 mm with the cap) |
| custom | the recipe's thread (`thread_d` or `socket_d`) × `socket_h` (else 55) | `thread_pitch` or `socket_pitch` | A60, P45 under 34 mm | | |

The holder sizes are the ones the recipes already use (the shade's thread on the body, the
Thread pendant's body heights); the bulbs are the IEC 60064 shapes, sizes as manufacturers
list them (worth checking against a datasheet before designing to the millimetre). The cable
is 6 mm across.

**The thread:** the holder's body carries the thread the shade screws onto, at the recipe's
pitch (the shade's layer height: one bead a turn), in the 3D view, the render and the Plan:
right-handed, one pitch a turn, its crests at the body's diameter (where the shade's bead runs),
its roots about half a pitch in, all along the body (a shade's straight start can sit anywhere
on it), fading in and out over half a turn at each end (`socketThread`, `threadRadius` in `fixture.js`). The
Plan's section shows its teeth, half a pitch higher on the left than on the right (the cut is
half a turn round).

**The bulb's screw:** the bulb has its Edison screw cap (IEC 60061-1): E27 26.5 mm across, 7
turns an inch (3.63 mm pitch), 20 mm threaded; E14 14 mm, 9 turns an inch (2.82 mm), 15 mm;
then the black insulator and the contact on top (`capThread`, `capTop`). Screwed in, all of it
but its bottom 3 mm is inside the holder; taken apart, the bulb comes off with it showing.

**Where it goes:** on the straight section that screws onto the socket: the part whose beads run
round the holder's body (within three quarters of a bead of it). The socket always points down:
its face (where the bulb comes out) is at that section's lower end as the lamp hangs, the body
rising from there to the cable.

**Which way it hangs:** the shades down past the bulb (a pendant, the default) or up away from it,
the **Hangs ↓ / ↑** button in the Render view's row. The project is turned over for it if need
be: the view setting is kept with the project's colours (in its file too); the recipe is
untouched. The 3D view and the Plan show the project as it prints (throat on the bed), the
fittings placed as the lamp hangs; the render shows it hanging.

## Three steps: prepare, set up, render

Nothing renders by itself: a slider or a camera move never starts the GPU. (Rendering live,
every change restarted the path tracer at full size; on a Mac a big picture with many bounces
ran the GPU out of memory and froze the browser.)

1. **Prepare** takes a snapshot of the project as it is: each printed part's toolpaths swept into
   solid beads at their real line width and layer height (the 3D view's *Line width* beads,
   `beadGeometry`, so the layer lines are real round faces), the fittings and the hidden parts
   (left out, as the brim is), each part where the assembly row has it: **taken apart** (part
   way or Exploded), every shade is rendered turned and moved off its thread as the 3D view shows
   it, and the fittings come apart too (`fixtureTakenOut`): top to bottom as it hangs, it reads
   **socket, shades, bulb**. The socket (and its cable) goes up until its face is 20 mm above the
   highest shade, clear of them all; the bulb unscrews from it and goes down until its cap is
   20 mm below the lowest shade, the whole bulb under them. Both follow the assembly row, from
   where the shades are taken apart to at the time. The 3D view and the Plan show them so too.
   Taken apart, Prepare switches the light off (the lamp's to be seen, not lit; it can be turned
   on again); the bulb shows as opal glass, and the holder's thread shows.
   The assembly row stays under the view in the Render view. Slow, once: changing the project
   afterwards changes nothing in the
   render until it's prepared again (the Prepare button lights up when it's out of date). The
   parts' colours and materials aren't in the snapshot: they change without preparing again.
2. **Set up** the light, camera, room, materials and quality in the side panel's **Render** tab
   (it shows while the Render view does): fold-outs as in the settings window, every number
   typed or slid, so a render's settings can be set again exactly. The
   canvas shows a **draft**: the scene in plain grey, drawn at once, to frame the picture. Grey on
   purpose: it's never mistaken for the render.
3. **Preview** renders small and quick (640 px, 32 samples by default); **Render** renders the
   picture (1920 px, 512 samples by default); both with the same bounces (12 by default), so the
   preview shows the light as the render will. **Stop** stops either; the picture so far stays and
   **PNG** saves it. The status line says what it's doing: compiling the shaders (the first time,
   a while on some GPUs), samples so far, the picture's pixels, the time.

What's on the canvas is only ever the draft or the path tracer's picture: no quick stand-in
while the shaders compile or the camera moves (that stand-in drew transmission differently, so
the same scene sometimes looked like another material), and the noise is the same every time
(stable noise): the same settings give the same picture.

Sizes are the picture's longer side in pixels (320 to 3840), whatever the screen; the GPU's work
is cut into tiles so each pass is small (by pixels × bounces, the see-through ones too; smaller
still on a Mac, iPhone or iPad, whose GPU resets a page whose work runs long), which keeps the
page and the GPU responsive. Then it's paced by the GPU itself: no new piece until the last is
done (a fence), and the tiles resized between samples so each piece takes about 30 ms, whatever
the scene. (Safari, once a page has reset the GPU, keeps WebGL off for that page until it's quit
and opened again: the status line says to restart the browser.) If the GPU is reset anyway (the browser takes the render's graphics
context away), the status line says so and Prepare starts afresh. The render asks the browser for the high-performance GPU (on a laptop with two).

On a Mac (only), the render learns: each GPU reset during a render makes its pieces 4 × smaller
from then on (remembered in the browser, `easycad.renderCare`, up to 64 ×), and each finished
picture makes them a little larger again (× 0.85); a Mac may also cut a sample into up to 32 × 32
tiles (others 16 × 16). Smaller pieces are slower (one piece a frame) but don't trip the Mac's
GPU watchdog. Other computers' pacing is unchanged.

## The camera

The **Camera** group has where it looks from (Front, ¾, Side, Below, Above, Top: **Fit** frames the lamp from
there; the view toolbar's 3D, Front, Side and Top snap the render's camera to ¾, Front, Side and
Top, framed) and the **lens** (24 to 135 mm, as on a full-frame camera: a wide lens up close bends the
lamp's lines, a long one far off keeps them straight). A new lens keeps the framing (the camera
moves to keep the same width in view). Orbit, pan and zoom in the view as in the 3D view; where
the camera is left is kept with the project (in its file too), so a render can be done again.

## The room

The **Room** group puts surfaces round the lamp, lit by it and in the picture (and the PNG):

- **Wall:** behind the lamp (on the far side from the Front view), its distance from the lamp's
  axis. 0 cuts straight through the lamp (a section: the half behind the wall is hidden, the
  wall shows how the light leaves the shade); less than 0 puts it in front of the axis. When the
  wall cuts through the bulb, the bulb's light comes from its half in front of the wall (half
  the glass radius out), so the wall isn't lit from behind. Through the axis, though, the
  bulb's light only skims the wall: every bump of the shade's cut edge throws a long shadow (a
  fan of streaks, worst with a pattern), as a lamp cut in half against a wall would. For the
  shade's light on a wall, **Wall measured from: the lamp's back** puts it just behind the
  lamp (20 mm from its back when chosen; 0 touches it);
- **Floor:** under the lamp (a floor or a table), its distance below the lamp's lowest point;
- **Ceiling:** above the lamp, its distance above the socket's top; the cable ends there.

Moving a distance turns its surface on. One colour for all three (a matte paint). With any of
them on, the picture isn't transparent where they are: the light pool on the wall and the
shadows are the point.

Each surface is solid from the room's side only (it faces the lamp). A camera outside the room
(above the ceiling, below the floor, behind the wall) looks through it, as an architect's
cutaway; the light inside still bounces off it. (Solid from both sides, such a surface hid the
lamp, though the draft, which draws front faces only, showed it.)

## The denoiser

A path tracer's picture is grainy until it has many samples; a see-through shade most of all
(the light reaching it through the plastic is found by chance). **Denoise** (beside PNG, once a
Preview or Render is done or stopped) runs Intel **Open Image Denoise**, the denoiser production
renderers use, a neural network trained on rendered pictures, on the GPU through WebGPU
(`oidn-web`). Off until pressed; **Denoise by itself** in the Quality group runs it on every
finished picture.

- It's given the picture in full float (before tone mapping) and two guides drawn from the same
  camera: each surface's colour (albedo) and its normal. The guides are drawn, not path traced,
  so they're clean, and the network is the one trained for clean guides (`rt_hdr_calb_cnrm`):
  it keeps what the normals show, the beads' round faces, so **the layer lines stay**. (The first
  version used the network for noisy guides, which smoothed them away with the noise.) On the
  layer-line test (a frosted lamp at 640 px, 24 samples) the brightness changes about 4 times (3.8–4.5) more
  from row to row than from column to column once denoised: the lines, sharper than through the
  grain (1.1, grain is the same both ways).
- **Fireflies first.** A path that finds the bulb through a lens of plastic now and then brings
  back a pixel tens of times too bright: the display clips it to a white speck, but the network
  spread its light round it, blotches and a picture brighter than it looks as rendered. Before
  the network, a pixel brighter than 4 × all but one of its neighbours is brought down to that
  (`web/js/denoise-prep.js`; an edge, the bulb's outline, has a bright neighbour and stays).
- **Denoiser: Detailed** (the default) is Open Image Denoise's large network
  (`rt_hdr_calb_cnrm_large`, 7.7 MB, loaded the first time): slower, it keeps fine detail
  better. **Fast** is the standard one (1.8 MB). Changing it denoises the picture again.
- **Denoise: detail kept** (35 % by default): what the guides don't show, the network takes for
  noise: the light a see-through shade's layer lines and a woven pattern bend, the glints along
  the beads. What it took away, smoothed over a few pixels (the grain mostly cancels there, a
  line several pixels long stays), is added back by this much. 0 %: the network's picture.
- **Denoise strength** (Quality) blends the picture as rendered back in: 100 % all denoised,
  less keeps some grain and every bit of detail. Strength and detail change the picture at once,
  without rendering again. (They didn't for a while: moving the camera kept its place in the
  project but not in the renderer, so the next change of anything looked like a new camera and
  cleared the picture.)
- Once denoised, the button switches between the denoised picture and the one as rendered; the
  PNG saves whichever shows.
- The alpha is denoised too where a shade is see-through over the transparent background.
- The network loads the first time it's needed.
- It needs enough to go on: at a few hundred samples nothing is guessed; at 16–32 on a tiny
  picture (a layer a pixel high) fine detail goes soft.
- Where the browser has no WebGPU, the path tracer's own edge-aware smoothing is used instead
  (the status line says so): lighter, and it softens more.

## The light

- **Light on:** the bulb is the main light: a light at the glass's centre (its colour
  temperature, 2700 K by default, and its output in lumens, 806 for a 60 W bulb), the frosted
  glass glowing as bright as the light it lets out (and not shading the light inside it). A
  little light from round the lamp (**Room**, 6 % by default) keeps the dark side from going
  black.
- **Light size** (100 % by default): the light shines from a sphere the size of the glass (a
  little inside it), as a frosted bulb glows over its whole glass. From a point, the shade's
  layer lines and patterns fell on the wall as sharp as from a slide projector; from the
  glass's size they blur as they do in a room, the more the further the wall. 0 % is a point:
  a clear bulb's filament, which does throw them sharp. (The path tracer's point lights have no
  size: a patch in the vendored library adds one, `web/vendor/three-gpu-pathtracer/README.txt`.)
  With the wall through the bulb, the sphere stays in the bulb's half in front of it.
- **Light off:** daylight from round the lamp only (a bright sky over a darker floor); the bulb is
  plain opal glass. The bulb's light is taken out of the scene, not left at no power: a light of
  no power divides by zero in the light sampling, and on some GPUs (a Mac's) that blackened
  see-through shades.

The scene is in metres, so the light falls off with distance as it does in a room. AgX tone
mapping and the **Exp** slider (± stops) bring the bulb's glare and the shade's glow into one
picture.

## The materials: printed plastics

A **Material** group per printed part has its **colour** and how **solid** it is (the same as the
3D view's custom colours), and its **plastic** (`web/js/materials.js`):

| Plastic | Matte | Coat | Sheen | Silk | IOR | Depth |
|---|---|---|---|---|---|---|
| PLA | 40 % | | | | 1.46 | 4 mm |
| PLA matte | 70 % | | 30 % | | 1.46 | 3 mm |
| PLA silk | 22 % | 60 % | | 45 % | 1.46 | 2 mm |
| PETG | 18 % | 30 % | | | 1.57 | 12 mm |
| PETG frosted | 55 % | | | | 1.57 | 8 mm |
| PC clear | 6 % | 60 % | | | 1.585 | 40 mm |
| PP | 45 % | | 20 % | | 1.49 | 6 mm |
| ABS / ASA | 50 % | | | | 1.53 | 3 mm |
| TPU | 55 % | | 40 % | | 1.5 | 5 mm |
| Wood fill | 85 % | | 35 % | | 1.5 | 1 mm |
| Stone / marble fill | 60 % | 10 % | | | 1.5 | 1.5 mm |

Each value can be set for the part (↺ goes back to the plastic's):

- **Solid:** 100 % bounces all the light it doesn't absorb (a white shade reflects the bulb's
  light round the inside and out of the opening); less lets that much pass into the plastic
  (transmission = 1 − solid), where it bends at each bead's round face and comes out again: the
  layer lines show as bright and dark bands, as on a print held to the light.
- **Matte:** how rough the surface is: glossy reflections to a soft sheen, and how much the light
  passing through spreads (printed walls frost it).
- **Coat:** a clear gloss layer over it (silk and clear plastics).
- **Sheen:** a soft, velvety edge (matte and filled plastics).
- **Silk:** the metallic shine of silk PLA.
- **Depth:** how far light travels in the plastic before it has the plastic's colour: short for
  deeply coloured or filled plastic, long for clear.
- **IOR:** the refractive index, how much the beads bend the light.

The plastic and its values are kept per part with the custom colours (in the project file too).
Colour and material changes need no Prepare.

## The picture

The background stays transparent (the checkerboard behind the view), unless the room's surfaces
fill it; the lamp's own pixels are solid where it's opaque and partly transparent where the
background shows through a clear shade, so it composites over any environment. The bulb is opal
glass: solid in the picture, glowing when on. **PNG** saves the picture as it is, at its size.

On a phone it's slow (a lamp of a million triangles is fine on a laptop's GPU), but preparing,
setting up and a small preview work anywhere.

## The batch

The Render tab's **Batch** group renders a list of setups one after another (overnight, say):

- **Save current settings for batch** keeps one setup under the name typed (or the next free
  number): the render's settings, the camera as it is, and the project as it is now (its beads
  where the assembly row has them, the parts' looks, the fittings, hidden parts left out). Then
  anything can change: settings, the camera, the project itself (another recipe value, another
  project), Prepare and Preview again, and save the next setup. Setups from different projects
  can share a batch.
- The setups are kept in the browser (IndexedDB, `web/js/batch.js`: the beads are too big for
  localStorage), so they survive a reload; **×** takes one out.
- **Saving the project file keeps its batch plan** (a `#~ batch` line): the setups saved from
  this project, each with its render settings and what makes its picture (the recipe with its
  values, the parts' looks and how it hangs, the parts hidden, how far apart; each recipe once),
  but not the beads. Opening the file on another computer adds its setups to the batch there
  (ones it has already are left out); the batch makes their beads from the recipe when it gets
  to them. So the plan can be made on one computer and rendered on another.
- **Batch render** prepares each setup, renders it at its Render size and samples, downloads the
  picture as rendered, then denoises it and downloads that too (`batch-<start>-<name>.png` and
  `…-denoised.png`, so a batch's pictures sort together in the downloads folder). The status line
  says which setup it's on. A GPU reset retries the setup once (a Mac then takes smaller pieces),
  and a setup that still fails is named at the end; the rest go on. **Stop** (or Stop batch)
  stops the batch.
- While it runs, the screen is kept on where the browser allows (Wake Lock): a sleeping screen or
  a hidden tab stops the page drawing, and the render with it. Changes to the settings meanwhile
  are kept for later and leave the batch alone. A browser may ask once to allow several downloads.

## How it's built

- **three-gpu-pathtracer** 0.0.27 (MIT, by Garrett Johnson; with **three-mesh-bvh** 0.9.18):
  an unbiased path tracer in a WebGL 2 shader, on the three.js scene the app already draws, with
  three's physical materials (transmission, roughness, attenuation, emission), point lights and
  environment light. 0.0.27 is the newest that works with our three.js 0.186 (0.0.28 needs
  0.187); vendored in `web/vendor/`, loaded only when the Render view first opens
  (`web/js/render.js`).
- The bead sweep winds its faces inward (the 3D view draws both sides); the render turns them
  outward, as the path tracer reads in and out of a solid from them.
- **One solid wall** (the "solidization"): beads of consecutive turns touch at their flat tops
  and bottoms. Left as surfaces, light crossing from one turn into the next would refract out of
  one bead and into the next, and be trapped in each turn by total internal reflection: a
  see-through shade came out dark. So the faces where a turn sits on the next (`web/js/wall.js`
  finds them: a bead one layer height up, within half a bead across) get a material with the
  same refractive index on both sides: light crosses them unbent, tinted as it goes. The wall's
  outside is then its beads' round faces, inside and out, where the light bends: the layer lines'
  lensing. The first turn's bottom, the last turn's top and any step where a turn sits out past
  half a bead stay real faces.
- **Only as far as the next turn covers it.** On a sloping wall sliced by height (a cone, a
  fillet), each turn sits partly beside the last: of a bead's flat top, only the part under the
  next bead is inside the wall; the rest is the wall's outside. `touching` also says where the
  beads above and below sit across, and the bead's section gets a point on its flat top and its
  flat underside where that cover ends; only the covered part goes inside. (Whole flat faces
  went inside at first: the uncovered strips were clear windows, smooth whatever the plastic's
  roughness, and a fillet near the bulb, which the light meets square to those strips, glowed.
  Sliced along the surface, the beads tilt with the wall and cover each other's flat faces
  whole, which is why it didn't show there.)
- **Up is the bead's up.** Sliced along the surface, each turn sits on the last along the
  wall, not straight up: `touching` looks one layer height along each point's up direction
  (the toolpath's `ups`). Looking straight up, it found no turn on a sloping wall sliced along
  the surface, every face between turns stayed a real one, and a clear shade went black (the
  light trapped in each turn, as at first).
- **One material per mesh.** The path tracer merges the scene into one geometry, numbers the
  materials by mesh (a group per mesh into the flat list of every mesh's materials) and keeps a
  material per vertex; a mesh with two materials gives its neighbours the wrong ones. So each part
  is two meshes (its beads; the faces inside its wall, with vertices of their own), and the
  merged geometry's groups (which it adds without clearing) are cleared before each scene. This
  was the "same scene, another material" of the first version.
- The faces inside the wall are a thin film (light crosses unbent) in a see-through part, and
  solid in a solid one (see-through there, they let light slip through the seams where one
  turn's bead meets the next: faint rings of light inside an opaque shade). As a face of index 1
  between two solids, the path tracer's transmission made NaNs and blackened the pixels behind
  a see-through shade.
- When the 3D view loses its GPU context (a phone short of memory), the prepared render is
  freed so the 3D view can be rebuilt (architecture §9.2): Prepare again afterwards.
- **oidn-web** 0.5.0 (MIT, by pissang; bundles @petamoriken/float16) runs Open Image Denoise's
  U-Net in WGSL compute shaders; the weights (`rt_hdr_calb_cnrm.tza`, Apache-2.0, Intel) come from
  the npm package `oidn-weights` 2.3.0-1. Both in `web/vendor/oidn-web/`, loaded only when the
  Render view denoises. The denoised picture is shown through the path tracer's own display pass,
  so its tone mapping and exposure are the same.
- **One line changed in the path tracer** (`web/vendor/three-gpu-pathtracer/README.txt`): the
  bulb casts no shadow (so its light inside gets out), and the library let any ray flagged as a
  shadow ray pass through such things, including the camera's own view through a rough
  see-through shade: the bulb went missing behind it. Now only rays that aren't still the
  camera's view through see-through surfaces pass.
- The library's WebGL path tracer is marked deprecated in favour of its WebGPU one; the WebGPU
  one is the upgrade once phones run WebGPU everywhere.

Not yet (on the roadmap): your own environment (an HDRI) for the light off, the room's light as a layer of its own over a transparent
background (a shadow and light catcher), a picture shape of its own (it's the view's), and
building the BVH in a worker (the first render after preparing blocks the page while it builds).
