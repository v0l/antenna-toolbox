# Changelog

All notable changes to Antenna Toolbox are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- A random wire on a 9:1 unun template.
- FDTD models can be built from meshed parts and imported STL files.
- Windows `.msi` and macOS `.dmg` installers with the Antenna Toolbox icon.

### Changed
- The 3D view is drawn by the egui_bench viewer shared with the other bench tools.
- FDTD runs on the sims field engine, and wire models are solved with a faster LU.

## [0.1.0] - 2026-09-24

First release, on the desktop and in the browser.

### Added
- 36 antenna templates, from Yagis, Moxons and log-periodics to helices, dishes, Vivaldis and
  printed antennas, each solved rather than looked up, with a cut sheet, drawing and feed detail.
- Wire antennas by the method of moments, with wire loss, insulation, lumped loads,
  transmission lines, networks, and free space, perfect or Sommerfeld ground. Sheet antennas by
  surface MoM, dishes by physical optics, printed antennas by FDTD on the GPU.
- NEC-2 deck import and export.
- Multiband SWR below 30 MHz, a design optimiser, matching network synthesis from standard
  parts, near-field RF exposure against ICNIRP 2020 and FCC limits, and characteristic modes.
- NanoVNA-H and H4 over USB, or Web Serial in the browser, with SOL calibration, a combined SWR,
  return loss, Smith and impedance display, and cable de-embedding.
- Radio paths over Copernicus GLO-30 terrain: profiles with earth curvature and Fresnel zone,
  Longley-Rice, ITU-R P.528 for aircraft, P.2108 clutter, P.452 time variability, P.533 HF
  skywave and coverage maps out to 500 km, with antenna patterns at both ends.
- Browser build with threads and WebGPU.
- Release builds with installers.

[Unreleased]: https://github.com/v0l/antenna-toolbox/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/v0l/antenna-toolbox/releases/tag/v0.1.0
