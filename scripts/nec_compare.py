#!/usr/bin/env python3
import sys

d = sys.argv[1] if len(sys.argv) > 1 else "/tmp/audit"
ours = {l.split()[0]: l.split()[1:] for l in open(f"{d}/ours.txt") if "skip" not in l}
for k, v in ours.items():
    o = open(f"{d}/{k}.out").read()
    row = o.split("ANTENNA INPUT PARAMETERS")[1].split("\n")[3].split()
    gains = []
    for line in o.split("RADIATION PATTERNS")[1].split("\n")[5:]:
        f = line.split()
        if len(f) < 8:
            break
        try:
            gains.append(float(f[4]))
        except ValueError:
            break
    zr, zi = float(row[6]), float(row[7])
    r, i, g = map(float, v)
    print(f"{k:<11} ours {r:7.1f}{i:+8.1f}j {g:5.2f} dBi | nec2 {zr:7.1f}{zi:+8.1f}j {max(gains):5.2f} dBi | dZ {abs(complex(r - zr, i - zi)):5.1f} dG {g - max(gains):+.2f}")
