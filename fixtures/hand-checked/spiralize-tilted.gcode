; Hand-checked fixture: spiralize through tilted layers (scripts/hand-checked-fixtures.py).
; The recipe (Klipper pellet printer, no start/end blocks, bed centre 125, 125):
;   easycad 0.1
;   base = polyline([[10, 0, 0], [0, 10, 0], [-10, 0, 0], [0, -10, 0]], closed: true)
;   top = base |> move(z: 2 + 0.1 * x)
;   layers = tween(base, top, range(0, 1, count: 5))
;   path = spiralize(layers, width: 1, flow: 5)
;   print path
; Each move's comment: its layer height (mean of the gaps at its two ends), length, bead
; area (w - h) h + pi (h/2)^2 and extrusion factor; E = area * length * factor (mm^3),
; F = round(5 * 60 / area).
G90
M83
G0 X135.000 Y125.000 Z0.000 F6000
G1 X125.000 Y135.000 Z0.125 E0.95670 F554 ; height 0.625 (ends 0.75, 0.5), length sqrt(200.015625) = 14.142688, area 0.541171, factor 0.125
G1 X115.000 Y125.000 Z0.125 E1.82869 F870 ; height 0.375 (ends 0.5, 0.25), length sqrt(200) = 14.142136, area 0.344822, factor 0.375
G1 X125.000 Y115.000 Z0.375 E3.04830 ; height 0.375 (ends 0.25, 0.5), length sqrt(200.0625) = 14.144345, area 0.344822, factor 0.625
G1 X135.000 Y125.000 Z0.750 E6.69901 F554 ; height 0.625 (ends 0.5, 0.75), length sqrt(200.140625) = 14.147107, area 0.541171, factor 0.875
; (the fan would come on here, after the ramp turn)
G1 X125.000 Y135.000 Z0.625 E7.65361 ; height 0.625 (ends 0.75, 0.5), length sqrt(200.015625) = 14.142688, area 0.541171, factor 1
G1 X115.000 Y125.000 Z0.375 E4.87728 F870 ; height 0.375 (ends 0.5, 0.25), length sqrt(200.0625) = 14.144345, area 0.344822, factor 1
G1 X125.000 Y115.000 Z0.875 E4.87956 ; height 0.375 (ends 0.25, 0.5), length sqrt(200.25) = 14.150972, area 0.344822, factor 1
G1 X135.000 Y125.000 Z1.500 E7.66079 F554 ; height 0.625 (ends 0.5, 0.75), length sqrt(200.390625) = 14.155940, area 0.541171, factor 1
G1 X125.000 Y135.000 Z1.125 E7.65601 ; height 0.625 (ends 0.75, 0.5), length sqrt(200.140625) = 14.147107, area 0.541171, factor 1
G1 X115.000 Y125.000 Z0.625 E4.87956 F870 ; height 0.375 (ends 0.5, 0.25), length sqrt(200.25) = 14.150972, area 0.344822, factor 1
G1 X125.000 Y115.000 Z1.375 E4.88337 ; height 0.375 (ends 0.25, 0.5), length sqrt(200.5625) = 14.162009, area 0.344822, factor 1
G1 X135.000 Y125.000 Z2.250 E7.66795 F554 ; height 0.625 (ends 0.5, 0.75), length sqrt(200.765625) = 14.169179, area 0.541171, factor 1
G1 X125.000 Y135.000 Z1.625 E7.66079 ; height 0.625 (ends 0.75, 0.5), length sqrt(200.390625) = 14.155940, area 0.541171, factor 1
G1 X115.000 Y125.000 Z0.875 E4.88337 F870 ; height 0.375 (ends 0.5, 0.25), length sqrt(200.5625) = 14.162009, area 0.344822, factor 1
G1 X125.000 Y115.000 Z1.875 E4.88869 ; height 0.375 (ends 0.25, 0.5), length sqrt(201) = 14.177447, area 0.344822, factor 1
G1 X135.000 Y125.000 Z3.000 E7.67749 F554 ; height 0.625 (ends 0.5, 0.75), length sqrt(201.265625) = 14.186812, area 0.541171, factor 1
G1 X125.000 Y135.000 Z2.000 E6.71337 ; height 0.625 (ends 0.75, 0.5), length sqrt(201) = 14.177447, area 0.541171, factor 0.875
G1 X115.000 Y125.000 Z1.000 E3.05543 F870 ; height 0.375 (ends 0.5, 0.25), length sqrt(201) = 14.177447, area 0.344822, factor 0.625
G1 X125.000 Y115.000 Z2.000 E1.83326 ; height 0.375 (ends 0.25, 0.5), length sqrt(201) = 14.177447, area 0.344822, factor 0.375
G1 X135.000 Y125.000 Z3.000 E0.95905 F554 ; height 0.625 (ends 0.5, 0.75), length sqrt(201) = 14.177447, area 0.541171, factor 0.125
