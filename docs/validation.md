# Validation

Each ported model is checked against the reference implementation it came from.

| Model | Reference | Test | Agreement |
|---|---|---|---|
| Longley-Rice ITM | NTIA ITM v1.4, SPLAT! 1.4.2 | `terrain::itm` | 0.01 dB on NTIA p2p cases |
| ITU-R P.528 | NTIA p528 | `terrain::p528` | 0.01 dB on 400 paths |
| ITU-R P.452-18 | Py452 validation set | `terrain::p452` | 2e-7 dB on 595 cases |
| ITU-R P.2108 | NTIA p2108 test data | `terrain::p2108` | every row |
| ITU-R P.533-14, P.372 | ITURHFProp 14.2 | `hf/tests/p533.rs` | 0.005 on 1200 cases (the printed precision) |
| Near fields | nec2c NE/NH cards | `solver/tests/validate.rs` | 1% |
| Characteristic modes | scipy `eig(X, R)` | `solver/tests/validate.rs` | 1e-3 |
| Matching networks | Pozar examples 5.1, 5.2 | `rf::synth` | textbook rounding |

## Known departures from the Recommendations

- P.533: the mirror-reflection height term G omits the `90.47·xr` term that the
  Recommendation has for `xr ≤ 3.7`. ITURHFProp omits it too, and the port keeps
  it out so the two agree.
- P.533: only analogue circuit reliability (BCR) is reported. The digital
  multimode and scattering terms are not ported.
- P.452: ΔN and N₀ are entered by hand instead of read from the ITU digital maps,
  which may not be redistributed.
