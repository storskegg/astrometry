"""Swiss Ephemeris reference house cusps and ayanamsas for the astrometry crate's tests.

Run by `cargo xtask fixtures houses` in the same throwaway virtual environment as
swisseph_fixtures.py: the Swiss Ephemeris is AGPL-3.0, so its code never enters the
repository; only these numbers do.

Arguments: the JPL DE440 file, the Swiss Ephemeris data directory, the instants file (one TT
Julian date per line), the output file.
"""

import os
import sys

import swisseph as swe

jpl, ephe, instants, out = sys.argv[1:5]
swe.set_ephe_path(ephe)
swe.set_jpl_file(os.path.basename(jpl))

with open(instants) as f:
    jds = [float(line) for line in f if line.strip()]

systems = [b"P", b"K", b"E", b"W"]
lats = [float(x) for x in range(-66, 67, 6)] + [-65.9, -50.5, 50.5, 65.9]
epsilons = [23.4392911, 23.0, 24.0]
sids = [("FaganBradley", swe.SIDM_FAGAN_BRADLEY), ("Lahiri", swe.SIDM_LAHIRI)]

with open(out, "w") as f:
    f.write(
        "# Swiss Ephemeris %s house cusps and ayanamsas. Made by `cargo xtask fixtures houses`;\n"
        "# don't edit.\n"
        "# house\tsystem\tarmc\tlat\teps\tcusp1..cusp12 (degrees)\n"
        "# ayanamsa\tname\tjd_tt\twith nutation\twithout nutation (degrees)\n"
        "# sidereal\tname\tbody\tjd_tt\tsidereal longitude, JPL mode (degrees)\n" % swe.version
    )
    for eps in epsilons:
        for lat in lats:
            for armc in range(0, 360, 15):
                for hsys in systems:
                    try:
                        cusps, _ = swe.houses_armc(float(armc), lat, eps, hsys)
                    except swe.Error:
                        # Placidus and Koch are undefined at or beyond the polar limit; no row.
                        if hsys in (b"P", b"K") and abs(lat) >= 90.0 - eps:
                            continue
                        raise
                    f.write(
                        "house\t%s\t%d\t%.1f\t%.7f\t%s\n"
                        % (hsys.decode(), armc, lat, eps, "\t".join("%.9f" % c for c in cusps[:12]))
                    )
    for name, mode in sids:
        swe.set_sid_mode(mode, 0, 0)
        for jd in jds[::2]:
            with_nut = swe.get_ayanamsa_ex(jd, 0)[1]
            without = swe.get_ayanamsa_ex(jd, swe.FLG_NONUT)[1]
            f.write("ayanamsa\t%s\t%.9f\t%.9f\t%.9f\n" % (name, jd, with_nut, without))
        for body_name, body in [("Sun", swe.SUN), ("Moon", swe.MOON)]:
            for jd in jds[:12]:
                pos, ret = swe.calc(jd, body, swe.FLG_JPLEPH | swe.FLG_SIDEREAL)
                if (ret & swe.FLG_JPLEPH) != swe.FLG_JPLEPH:
                    sys.exit("%s at %s: fell back from JPL" % (body_name, jd))
                f.write("sidereal\t%s\t%s\t%.9f\t%.10f\n" % (name, body_name, jd, pos[0]))
