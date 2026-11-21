"""Swiss Ephemeris reference positions and events for fixed stars, for the astrometry crate's tests.

Run by `cargo xtask fixtures stars` in the same throwaway virtual environment as
swisseph_fixtures.py: the Swiss Ephemeris is AGPL-3.0, so its code never enters the
repository; only these numbers do.

The Swiss Ephemeris is given the crate's own catalogue (data/stars.tsv) through a fixed-star
file written into its data directory, so the comparison tests the reduction and not the
catalogue. The file's format and units are those of the Swiss Ephemeris documentation
(swisseph.htm and swephprg.htm, "Fixed stars"): name, nomenclature name, equinox, right
ascension h,m,s, declination d,m,s, proper motion in right ascension (times cos δ) and in
declination in mas/yr, radial velocity in km/s, parallax in mas, magnitude, DM zone and number.
Its coordinates are at epoch J2000, so each catalogue row is first carried to J2000 here, by the
rigorous space motion (a straight line in space), which keeps the star the same.

Arguments: the JPL DE440 file, the Swiss Ephemeris data directory, the instants file (one TT
Julian date per line), the output file, data/stars.tsv.
"""

import math
import os
import sys

import swisseph as swe

jpl, ephe, instants, out, catalogue = sys.argv[1:6]
swe.set_ephe_path(ephe)
swe.set_jpl_file(os.path.basename(jpl))

ARCSEC = math.pi / 180 / 3600
AU_KM = 149597870.7
J2000 = 2451545.0


def vec(ra, dec):
    return [math.cos(dec) * math.cos(ra), math.cos(dec) * math.sin(ra), math.sin(dec)]


def dot(a, b):
    return sum(x * y for x, y in zip(a, b))


def to_j2000(row):
    """The row's astrometry at J2000: (ra deg, dec deg, pm_ra mas/yr, pm_dec, rv km/s, plx mas)."""
    ra, dec = math.radians(float(row["ra"])), math.radians(float(row["dec"]))
    pmra, pmdec = float(row["pm_ra"]), float(row["pm_dec"])
    plx, rv, epoch = float(row["parallax"]), float(row["rv"]), float(row["epoch"])
    r0 = vec(ra, dec)
    east = [-math.sin(ra), math.cos(ra), 0.0]
    north = [-math.cos(ra) * math.sin(dec), -math.sin(ra) * math.sin(dec), math.cos(dec)]
    per_day = 1e-3 * ARCSEC / 365.25
    plx_rad = plx * 1e-3 * ARCSEC
    # Position and velocity in units of the distance at the epoch (and per day).
    v = [per_day * (pmra * e + pmdec * n) + rv * 86400 / AU_KM * plx_rad * r for e, n, r in zip(east, north, r0)]
    s = [r + (J2000 - epoch) * x for r, x in zip(r0, v)]
    k = math.sqrt(dot(s, s))
    r1 = [x / k for x in s]
    ra1 = math.atan2(r1[1], r1[0]) % (2 * math.pi)
    dec1 = math.asin(r1[2])
    east1 = [-math.sin(ra1), math.cos(ra1), 0.0]
    north1 = [-math.cos(ra1) * math.sin(dec1), -math.sin(ra1) * math.sin(dec1), math.cos(dec1)]
    return (
        math.degrees(ra1),
        math.degrees(dec1),
        dot(v, east1) / k / per_day,
        dot(v, north1) / k / per_day,
        dot(v, r1) / plx_rad * AU_KM / 86400,
        plx / k,
    )


def hms(deg):
    h = deg / 15
    m = (h - int(h)) * 60
    return "%02d,%02d,%012.9f" % (int(h), int(m), (m - int(m)) * 60)


def dms(deg):
    sign = "-" if deg < 0 else ""
    a = abs(deg)
    m = (a - int(a)) * 60
    return "%s%02d,%02d,%012.9f" % (sign, int(a), int(m), (m - int(m)) * 60)


rows = []
with open(catalogue) as f:
    lines = [l.rstrip("\n") for l in f if l.strip() and not l.startswith("#")]
header = lines[0].split("\t")
rows = [dict(zip(header, l.split("\t"))) for l in lines[1:]]
sirius = next(r for r in rows if r["hip"] == "32349")
with open(os.path.join(ephe, "sefstars.txt"), "w") as f:
    for r in rows:
        ra, dec, pmra, pmdec, rv, plx = to_j2000(r)
        f.write(
            "%s,HIP%s,ICRS,%s,%s,%.9f,%.9f,%.9f,%.9f,0.0,0,0\n"
            % (r["name"], r["hip"], hms(ra), dms(dec), pmra, pmdec, rv, plx)
        )
star = sirius["name"]

with open(instants) as f:
    jds = [float(line) for line in f if line.strip()]

sites = [("Luxor", 32.6391, 25.6995, 76.0), ("Heliopolis", 31.3075, 30.1293, 25.0)]
# The heliacal parameters: the atmosphere (pressure mbar, temperature deg C, relative humidity %,
# meteorological range km, 0: from the first three) and the observer (age, Snellen ratio).
atmosphere = (1013.25, 25.0, 20.0, 0.0)
observer = (36.0, 1.0, 1.0, 1.0, 0.0, 0.0)
rise = swe.CALC_RISE | swe.BIT_NO_REFRACTION | swe.BIT_DISC_CENTER
sett = swe.CALC_SET | swe.BIT_NO_REFRACTION | swe.BIT_DISC_CENTER
BCE = 2000000.0  # before this JD, use the Swiss Ephemeris' own files (DE440 starts in 1550)


def mode_of(jd):
    return ("swiss", swe.FLG_SWIEPH) if jd < BCE else ("jpl", swe.FLG_JPLEPH)


with open(out, "w") as f:
    f.write(
        "# Swiss Ephemeris %s, fixed star %s, from the crate's own catalogue (data/stars.tsv).\n"
        "# Made by `cargo xtask fixtures stars`; don't edit.\n"
        "# Mode jpl reads JPL DE440; swiss reads the Swiss Ephemeris' own files (before 1550,\n"
        "# where DE440 does not reach).\n"
        "# apparent: mode, jd_tt, right ascension, declination (of date, true equator and\n"
        "#   equinox), ecliptic longitude, latitude (of date), degrees.\n"
        "# icrs: mode, jd_tt, right ascension, declination, degrees: the apparent direction in\n"
        "#   the axes of the ICRS (flags J2000 and ICRS: no precession or nutation), with light\n"
        "#   deflection and aberration. It tests the reduction from the catalogue alone; the\n"
        "#   positions of date also rest on the precession and nutation models, which the Swiss\n"
        "#   Ephemeris sets itself (Vondrak 2011 and IAU 2000B by default).\n"
        "# rise, set: site, jd_ut at which the search starts, jd_ut of the event; the star's\n"
        "#   centre on the geometric horizon (no refraction), geocentric, mode as above.\n"
        "#   Sites: Luxor 25.6995 N 32.6391 E 76 m; Heliopolis 30.1293 N 31.3075 E 25 m.\n"
        "# heliacal: site, mode, astronomical year (Julian calendar before 1583), jd_ut at which the\n"
        "#   search starts (1 January 0h UT), heliacal rising: start of visibility, optimum,\n"
        "#   end (jd_ut), after the parameters used, which are:\n" % (swe.version, star)
    )
    f.write(
        "# heliacal parameters: atmosphere pressure %.2f mbar, temperature %.1f C, relative\n"
        "# humidity %.0f %%, meteorological range %.0f (computed from those); observer age %.0f,\n"
        "# Snellen ratio %.0f, naked eye; flags: ephemeris mode, HELFLAG_VISLIM_NOMOON (the Moon\n"
        "# not counted as a disturbance, so the date is the epoch's, not the year's); event\n"
        "# HELIACAL_RISING.\n" % (atmosphere + observer[:2])
    )

    def apparent(jd):
        mode, flag = mode_of(jd)
        eq, name, ret = swe.fixstar2(star, jd, flag | swe.FLG_EQUATORIAL)
        ec, name, ret2 = swe.fixstar2(star, jd, flag)
        if (ret & flag) != flag or (ret2 & flag) != flag:
            sys.exit("%s at %s: fell back from %s (flags %#x %#x)" % (star, jd, mode, ret, ret2))
        return "apparent\t%s\t%.9f\t%.10f\t%.10f\t%.10f\t%.10f\n" % (mode, jd, eq[0], eq[1], ec[0], ec[1])

    def icrs(jd):
        mode, flag = mode_of(jd)
        xx, name, ret = swe.fixstar2(star, jd, flag | swe.FLG_EQUATORIAL | swe.FLG_J2000 | swe.FLG_ICRS)
        if (ret & flag) != flag:
            sys.exit("%s at %s: fell back from %s (flags %#x)" % (star, jd, mode, ret))
        return "icrs\t%s\t%.9f\t%.10f\t%.10f\n" % (mode, jd, xx[0], xx[1])

    for jd in jds:
        f.write(apparent(jd))
    for jd in jds:
        f.write(icrs(jd))

    # Rising and setting, about twenty dates in 1800-2200 at each site.
    for name, lon, lat, alt in sites:
        for k in range(20):
            start = 2378596.5 + 7200.0 * k
            for kind, bits in (("rise", rise), ("set", sett)):
                res, tret = swe.rise_trans(start, star, bits, (lon, lat, alt), 0.0, 0.0, swe.FLG_JPLEPH)
                if res != 0:
                    sys.exit("%s %s at %s: result %d" % (kind, name, start, res))
                f.write("%s\t%s\t%.9f\t%.9f\n" % (kind, name, start, tret[0]))

    # Heliacal risings, every 10 years over 1800-2200, and five years near 1300 BCE.
    for name, lon, lat, alt in sites:
        for year in list(range(1800, 2201, 10)) + list(range(-1301, -1296)):
            cal = swe.GREG_CAL if year >= 1583 else swe.JUL_CAL
            start = swe.julday(year, 1, 1, 0.0, cal)
            mode, flag = mode_of(start)
            dret = swe.heliacal_ut(
                start, (lon, lat, alt), atmosphere, observer, star, swe.HELIACAL_RISING, flag | swe.HELFLAG_VISLIM_NOMOON
            )
            f.write("heliacal\t%s\t%s\t%d\t%.9f\t%.9f\t%.9f\t%.9f\n" % (name, mode, year, start, dret[0], dret[1], dret[2]))
