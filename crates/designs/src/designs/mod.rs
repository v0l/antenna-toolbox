mod biquad;
mod bowtie;
mod coil;
mod collinear;
mod corner;
mod dipole;
mod discmonopole;
mod discone;
mod dish;
mod groundplane;
mod helix;
mod lindenblad;
mod loop_;
mod moxon;
mod quad;
mod slimjim;
mod spiral;
mod vdipole;
mod vivaldi;
mod yagi;

use crate::Design;

pub static DESIGNS: [&Design; 21] = [
    &moxon::MOXON,
    &yagi::YAGI2,
    &yagi::YAGI3,
    &quad::QUAD,
    &biquad::BIQUAD,
    &helix::HELIX,
    &corner::CORNER,
    &dish::DISH,
    &vivaldi::VIVALDI,
    &loop_::LOOP,
    &bowtie::BOWTIE,
    &spiral::SPIRAL,
    &groundplane::GROUNDPLANE,
    &dipole::DIPOLE,
    &vdipole::VDIPOLE,
    &collinear::COLLINEAR,
    &slimjim::SLIMJIM,
    &discone::DISCONE,
    &discmonopole::DISCMONOPOLE,
    &coil::COIL,
    &lindenblad::LINDENBLAD,
];

pub fn by_id(id: &str) -> &'static Design {
    DESIGNS.iter().copied().find(|d| d.id == id).unwrap_or(DESIGNS[0])
}
