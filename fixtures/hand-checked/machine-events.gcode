; Hand-checked fixture: toolpath tuning steps (scripts/hand-checked-fixtures.py).
; The recipe (Klipper pellet printer, bed centre 125, 125; the moves between the
; start and end blocks, which are EasyGCode's and golden-tested elsewhere):
;   easycad 0.1
;   square = polyline([[0, 0], [20, 0], [20, 20], [0, 20]], closed: true)
;   two = toolpath(square, width: 1, layer: 0.5, flow: 4) |> stack(2)
;   tuned = adjust(two, speed: 1 - 0.5 * greater(x, 19), extrude: 1 + 0.5 * greater(y, 19))
;   ramped = accelerate(tuned, 10)
;   cooled = fan(ramped, level: 60 * greater(z, 0.6))
;   path = dwell(cooled, [[20, 20, 1]], 0.5)
;   print path
; Bead area (w - h) h + pi (h/2)^2 = 0.25 + pi / 16 mm^2 for every move.
G90
M83
G0 X125.000 Y125.000 Z0.500 F6000
G1 X145.000 Y125.000 Z0.500 E8.92699 F538 ; bottom side, full speed: length 20.000000, E = area x length x 1, F = 537.6952
G1 X145.000 Y145.000 Z0.500 E8.92699 F269 ; right side, half speed: length 20.000000, E = area x length x 1, F = 268.8476
G1 X144.624 Y145.000 Z0.500 E0.25204 F315 ; top side, ramp piece 1 of 8: length 0.376453, E = area x length x 1.5, F = 315.2517
G1 X144.247 Y145.000 Z0.500 E0.25204 F356 ; top side, ramp piece 2 of 8: length 0.376453, E = area x length x 1.5, F = 355.6519
G1 X143.871 Y145.000 Z0.500 E0.25204 F392 ; top side, ramp piece 3 of 8: length 0.376453, E = area x length x 1.5, F = 391.9093
G1 X143.494 Y145.000 Z0.500 E0.25204 F425 ; top side, ramp piece 4 of 8: length 0.376453, E = area x length x 1.5, F = 425.0854
G1 X143.118 Y145.000 Z0.500 E0.25204 F456 ; top side, ramp piece 5 of 8: length 0.376453, E = area x length x 1.5, F = 455.8533
G1 X142.741 Y145.000 Z0.500 E0.25204 F485 ; top side, ramp piece 6 of 8: length 0.376453, E = area x length x 1.5, F = 484.6719
G1 X142.365 Y145.000 Z0.500 E0.25204 F512 ; top side, ramp piece 7 of 8: length 0.376453, E = area x length x 1.5, F = 511.8706
G1 X125.000 Y145.000 Z0.500 E11.62617 F538 ; top side, piece 8 and the rest at full speed: length 17.364827, E = area x length x 1.5, F = 537.6952
G1 X125.000 Y125.000 Z0.500 E8.92699 ; left side, full speed: length 20.000000, E = area x length x 1, F = 537.6952
; hop: up to one layer above the new copy, over, down
G0 X125.000 Y125.000 Z1.500 F6000
G0 X125.000 Y125.000 Z1.500 F6000
G0 X125.000 Y125.000 Z1.000 F6000
M106 S153 ; fan 60 % from layer 2 on
G1 X145.000 Y125.000 Z1.000 E8.92699 F538 ; bottom side, full speed: length 20.000000, E = area x length x 1, F = 537.6952
G1 X145.000 Y145.000 Z1.000 E8.92699 F269 ; right side, half speed: length 20.000000, E = area x length x 1, F = 268.8476
G4 P500 ; dwell at (20, 20, 1)
G1 X144.624 Y145.000 Z1.000 E0.25204 F315 ; top side, ramp piece 1 of 8: length 0.376453, E = area x length x 1.5, F = 315.2517
G1 X144.247 Y145.000 Z1.000 E0.25204 F356 ; top side, ramp piece 2 of 8: length 0.376453, E = area x length x 1.5, F = 355.6519
G1 X143.871 Y145.000 Z1.000 E0.25204 F392 ; top side, ramp piece 3 of 8: length 0.376453, E = area x length x 1.5, F = 391.9093
G1 X143.494 Y145.000 Z1.000 E0.25204 F425 ; top side, ramp piece 4 of 8: length 0.376453, E = area x length x 1.5, F = 425.0854
G1 X143.118 Y145.000 Z1.000 E0.25204 F456 ; top side, ramp piece 5 of 8: length 0.376453, E = area x length x 1.5, F = 455.8533
G1 X142.741 Y145.000 Z1.000 E0.25204 F485 ; top side, ramp piece 6 of 8: length 0.376453, E = area x length x 1.5, F = 484.6719
G1 X142.365 Y145.000 Z1.000 E0.25204 F512 ; top side, ramp piece 7 of 8: length 0.376453, E = area x length x 1.5, F = 511.8706
G1 X125.000 Y145.000 Z1.000 E11.62617 F538 ; top side, piece 8 and the rest at full speed: length 17.364827, E = area x length x 1.5, F = 537.6952
G1 X125.000 Y125.000 Z1.000 E8.92699 ; left side, full speed: length 20.000000, E = area x length x 1, F = 537.6952
