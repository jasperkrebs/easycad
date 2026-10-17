; Hand-checked fixture: machine events on the filament printer (scripts/hand-checked-fixtures.py).
; The recipe (filament printer: nozzle 210 C, part fan 100 %, filament 1.75 mm, bed centre
; 125, 125; the moves between the start and end blocks, which are EasyGCode's):
;   easycad 0.1
;   square = polyline([[0, 0], [20, 0], [20, 20], [0, 20]], closed: true)
;   layer1 = toolpath(square, width: 0.6, layer: 0.3, flow: 6) |> temperature(nozzle: 230, at_end: true)
;   primer = toolpath(line([-20, -20], [-10, -20]), width: 0.6, layer: 0.3, speed: 20)
;     |> wait_temperature(nozzle: 230)
;     |> extrusion_override(80, at_end: true)
;     |> speed_override(125, at_end: true)
;   layer2 = toolpath(square |> move(z: 0.3), width: 0.6, layer: 0.3, flow: 6)
;     |> speed_override(100, at_end: true)
;     |> extrusion_override(100, at_end: true)
;     |> temperature(at_end: true)
;   path = join([layer1, primer, layer2], clear: 1)
;   print path
; Bead area (w - h) h + pi (h/2)^2 = 0.09 + 0.0225 pi mm^2 for every move.
G90
M83
G0 X125.000 Y125.000 Z0.300 F6000
G1 X145.000 Y125.000 Z0.300 E1.33611 F2240 ; layer 1, side 1: length 20.000000, E = area x length / filament area, F = 2240.3966
G1 X145.000 Y145.000 Z0.300 E1.33611 ; layer 1, side 2: length 20.000000, E = area x length / filament area, F = 2240.3966
G1 X125.000 Y145.000 Z0.300 E1.33611 ; layer 1, side 3: length 20.000000, E = area x length / filament area, F = 2240.3966
G1 X125.000 Y125.000 Z0.300 E1.33611 ; layer 1, side 4: length 20.000000, E = area x length / filament area, F = 2240.3966
M104 S230 ; temperature(nozzle: 230, at_end: true)
G0 X125.000 Y125.000 Z1.000 F6000 ; hop to the primer: up to 1
G0 X105.000 Y105.000 Z1.000 F6000 ; over
G0 X105.000 Y105.000 Z0.300 F6000 ; down onto the primer's start
M109 S230 ; wait_temperature(nozzle: 230)
G1 X115.000 Y105.000 Z0.300 E0.66805 F1200 ; primer: length 10.000000, E = area x length / filament area, F = 1200.0000
M221 S80 ; extrusion_override(80, at_end: true)
M220 S125 ; speed_override(125, at_end: true)
G0 X115.000 Y105.000 Z1.000 F6000 ; hop to layer 2: up to 1
G0 X125.000 Y125.000 Z1.000 F6000 ; over
M106 S255 ; the automatic fan, right before layer 2's first point
G0 X125.000 Y125.000 Z0.600 F6000 ; down onto layer 2's start
G1 X145.000 Y125.000 Z0.600 E1.33611 F2240 ; layer 2, side 1: length 20.000000, E = area x length / filament area, F = 2240.3966
G1 X145.000 Y145.000 Z0.600 E1.33611 ; layer 2, side 2: length 20.000000, E = area x length / filament area, F = 2240.3966
G1 X125.000 Y145.000 Z0.600 E1.33611 ; layer 2, side 3: length 20.000000, E = area x length / filament area, F = 2240.3966
G1 X125.000 Y125.000 Z0.600 E1.33611 ; layer 2, side 4: length 20.000000, E = area x length / filament area, F = 2240.3966
M220 S100 ; speed_override(100, at_end: true)
M221 S100 ; extrusion_override(100, at_end: true)
M104 S210 ; temperature(at_end: true): the printer's own
