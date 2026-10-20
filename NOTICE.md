# Notices

astrometry and horae are licensed under the Apache License 2.0 (see LICENSE). It builds in the following
data, under their own terms.

| Data | Where | Source | Terms |
|---|---|---|---|
| Planetary and lunar ephemeris, 1800–2200 | `data/de440-1800-2200.bin` | JPL's DE440 (Park, Folkner, Williams and Boggs, *AJ* 161, 105, 2021), as distributed by JPL and NAIF; trimmed and re-encoded by this project (see `src/slice.rs`) and validated against the original | redistributed as NAIF's rules for SPICE kernels allow |
| Planetary theory | `data/vsop87` | VSOP87E (P. Bretagnon and G. Francou, *A&A* 202, 309, 1988), from IMCCE | published theory; no licence stated |
| Lunar theory | `data/elpmpp02` | ELP/MPP02 (J. Chapront and G. Francou, *A&A* 404, 735, 2003), the authors' series from the Paris Observatory's distribution | published theory; no licence stated |
| Nutation series | `data/iers-tab5.3*.txt` | IAU 2000A_R06, IERS Conventions (2010), chapter 5 | IERS |
| Earth rotation and time | `data/*.tsv`, `data/leap-seconds.list` | JPL Horizons' ΔT, IERS EOP 20 C04, USNO, the IANA leap second list | public data |

The tests compare against numbers from JPL Horizons and from the Swiss Ephemeris
(`tests/data`). The Swiss Ephemeris is AGPL-3.0 or commercial, so none of its code is here:
`cargo xtask fixtures` runs it in a throwaway environment under `target/`, and only its
outputs, the positions at fixed instants, are kept.
