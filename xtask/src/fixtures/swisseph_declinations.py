"""Swiss Ephemeris reference declinations for the astrometry crate's tests.

Run by `cargo xtask fixtures declinations` in the same throwaway virtual environment as
swisseph_fixtures.py: the Swiss Ephemeris is AGPL-3.0, so its code never enters the
repository; only these numbers do.

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
    ("Uranus", swe.URANUS),
    ("Neptune", swe.NEPTUNE),
    ("Pluto", swe.PLUTO),
    ("MeanNode", swe.MEAN_NODE),
    ("TrueNode", swe.TRUE_NODE),
    ("MeanApogee", swe.MEAN_APOG),
]
swe.set_ephe_path(ephe)
swe.set_jpl_file(os.path.basename(jpl))

with open(instants) as f:
    jds = [float(line) for line in f if line.strip()][::2]

flags = swe.FLG_JPLEPH | swe.FLG_EQUATORIAL | swe.FLG_SPEED
with open(out, "w") as f:
    f.write(
        "# Swiss Ephemeris %s apparent geocentric declinations at TT instants, JPL (DE440) mode:\n"
        "# declination (degrees) and its speed (degrees/day). Made by\n"
        "# `cargo xtask fixtures declinations`; don't edit.\n"
        "# body\tjd_tt\tdeclination\tspeed\n" % swe.version
    )
    for name, body in bodies:
        for jd in jds:
            (_, dec, _, _, dec_speed, _), ret = swe.calc(jd, body, flags)
            if (ret & swe.FLG_JPLEPH) != swe.FLG_JPLEPH and body not in (
                swe.MEAN_NODE,
                swe.TRUE_NODE,
                swe.MEAN_APOG,
            ):
                sys.exit("%s at %s: fell back from JPL (flags %#x)" % (name, jd, ret))
            f.write("%s\t%.9f\t%.10f\t%.10f\n" % (name, jd, dec, dec_speed))
