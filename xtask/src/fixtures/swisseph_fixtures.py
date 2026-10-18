"""Swiss Ephemeris reference positions for the astrometry crate's tests.

Run by `cargo xtask fixtures` in a throwaway virtual environment under target/: the Swiss
Ephemeris is AGPL-3.0, so its code never enters the repository; only these numbers do.

Arguments: the JPL DE440 file, the Swiss Ephemeris data directory, the instants file (one TT
Julian date per line), the output file.
"""

import os
import sys

import swisseph as swe

jpl, ephe, instants, out = sys.argv[1:5]
bodies = [
    ("Sun", swe.SUN),
    ("Moon", swe.MOON),
    ("Mercury", swe.MERCURY),
    ("Venus", swe.VENUS),
    ("Mars", swe.MARS),
    ("Jupiter", swe.JUPITER),
    ("Saturn", swe.SATURN),
    ("MeanNode", swe.MEAN_NODE),
    ("TrueNode", swe.TRUE_NODE),
]
modes = [("jpl", swe.FLG_JPLEPH), ("swiss", swe.FLG_SWIEPH)]
swe.set_ephe_path(ephe)
# The JPL file is looked up by name in the data directory.
swe.set_jpl_file(os.path.basename(jpl))

with open(instants) as f:
    jds = [float(line) for line in f if line.strip()]

with open(out, "w") as f:
    f.write(
        "# Swiss Ephemeris %s apparent geocentric positions at TT instants, ecliptic of date:\n"
        "# longitude, latitude (degrees), distance (au), speed in longitude (degrees/day).\n"
        "# Mode jpl reads JPL DE440; swiss reads its own files. Made by `cargo xtask fixtures`;\n"
        "# don't edit.\n# mode\tbody\tjd_tt\tlon\tlat\tdist\tspeed\n" % swe.version
    )
    for mode, flag in modes:
        for name, body in bodies:
            for jd in jds:
                (lon, lat, dist, speed, _, _), ret = swe.calc(jd, body, flag | swe.FLG_SPEED)
                if (ret & flag) != flag and body not in (swe.MEAN_NODE, swe.TRUE_NODE):
                    sys.exit("%s %s at %s: fell back from %s (flags %#x)" % (mode, name, jd, mode, ret))
                f.write("%s\t%s\t%.9f\t%.10f\t%.10f\t%.12f\t%.10f\n" % (mode, name, jd, lon, lat, dist, speed))
    f.write("# Swiss Ephemeris ΔT (TT - UT), seconds, on the first of each month at 0h UT\n")
    for year in range(1800, 2201):
        for month in range(1, 13):
            jd = swe.julday(year, month, 1, 0.0, swe.GREG_CAL)
            f.write("deltat\t%d\t%d\t%.6f\n" % (year, month, swe.deltat(jd) * 86400))

    # Events, by bisection on the Swiss Ephemeris' own apparent longitudes (JPL mode): new and
    # full moons, and the Moon's ingresses into signs, to about a millisecond.
    def lon(jd, body):
        return swe.calc(jd, body, swe.FLG_JPLEPH)[0][0]

    def wrap(a):
        return (a + 540.0) % 360.0 - 180.0

    def crossings(f, start, stop, step):
        out = []
        t, prev = start, f(start)
        while t < stop:
            u = t + step
            cur = f(u)
            if prev < 0 <= cur and cur - prev < 180:
                lo, hi = t, u
                while hi - lo > 1e-8:
                    mid = (lo + hi) / 2
                    if f(mid) < 0:
                        lo = mid
                    else:
                        hi = mid
                out.append((lo + hi) / 2)
            t, prev = u, cur
        return out

    f.write("# Events (TT), by bisection on the Swiss Ephemeris' JPL-mode longitudes\n")
    spans = [(2461041.5, 2461406.5), (2396839.5, 2396929.5), (2506370.5, 2506460.5)]  # 2026; 1850 Jan-Mar; 2150 Jan-Mar
    for start, stop in spans:
        elong = lambda jd: wrap(lon(jd, swe.MOON) - lon(jd, swe.SUN))
        for jd in crossings(elong, start, stop, 0.5):
            f.write("event\tnew\t%.9f\n" % jd)
        for jd in crossings(lambda jd: wrap(elong(jd) - 180.0), start, stop, 0.5):
            f.write("event\tfull\t%.9f\n" % jd)
    for k in range(12):
        for jd in crossings(lambda jd: wrap(lon(jd, swe.MOON) - 30.0 * k), 2461314.5, 2461345.5, 0.25):
            f.write("event\tingress%d\t%.9f\n" % (k, jd))
