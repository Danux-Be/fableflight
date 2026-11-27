# Power lines

The game draws the power lines OpenStreetMap knows: the transmission lines
(`power=line`, on steel lattice towers) and the distribution lines
(`power=minor_line`, on poles), with their conductors and earth wires
(`src/power.rs`, `src/shaders/wires.wgsl`, `assets/blender/pylons.py`).

There are far too many to ship with the game (some 2.8 million lines and 39
million supports), so they are published as 266 regional bundles of 10 x 10
degrees (233 MB in all, 12 MB at most), and the game downloads the bundle
round the aircraft, once, into its cache (`~/.cache/fablegame/power/`).

## Where the game gets them

By default from the assets of the GitHub release tagged `power-data-1`
(`power::RELEASE`):

    https://github.com/Rolandjg/fableflight/releases/download/power-data-1/power_<lat>_<lon>.bin

`<lat>` and `<lon>` are the bundle's south-west corner in degrees, multiples
of 10 (`power_40_0.bin` covers 40-50 N, 0-10 E). A bundle that is not there
(a 404) means no lines there.

`--power-data DIR_OR_URL` takes them from a folder or another URL instead,
for testing a new build of the data.

## Building and publishing the bundles

1. Fetch every line with its geometry from QLever's OpenStreetMap endpoint
   (about two minutes, 1.4 GB); the query is at the top of
   `assets/mkpower.py`.
2. Cut them into bundles:

       python3 assets/mkpower.py lines.tsv bundles/

3. Publish them as the assets of a release (a release holds up to 1000
   files):

       gh release create power-data-1 --title "Power line data 1" \
           --notes "OpenStreetMap power lines for the game (© OpenStreetMap contributors, ODbL)." bundles/power_*.bin

To publish fresher data, make a new release (`power-data-2`) and change
`power::RELEASE`; caches keyed by the old file names keep working for the
old builds.

The data is © OpenStreetMap contributors, under the Open Database License;
the bundles are a derived database under the same licence.
