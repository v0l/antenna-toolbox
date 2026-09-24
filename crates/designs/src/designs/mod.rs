mod biquad;
mod bowtie;
mod coil;
mod collinear;
mod corner;
mod delta;
mod dipole;
mod discmonopole;
mod discone;
mod dish;
mod efhw;
mod fiveeighths;
mod folded;
mod groundplane;
mod hb9cv;
mod helix;
mod hentenna;
mod ifa;
mod invv;
mod jpole;
mod lindenblad;
mod loop_;
mod lpda;
mod monopole;
mod moxon;
mod ocf;
mod patch;
mod qfh;
mod quad;
mod sleeve;
mod slimjim;
mod spiral;
mod turnstile;
mod vdipole;
mod vivaldi;
pub mod yagi;

use crate::Design;

pub static DESIGNS: [&Design; 36] = [
    &moxon::MOXON,
    &yagi::YAGI,
    &lpda::LPDA,
    &hb9cv::HB9CV,
    &quad::QUAD,
    &biquad::BIQUAD,
    &helix::HELIX,
    &corner::CORNER,
    &dish::DISH,
    &vivaldi::VIVALDI,
    &loop_::LOOP,
    &delta::DELTA,
    &hentenna::HENTENNA,
    &folded::FOLDED,
    &invv::INVV,
    &efhw::EFHW,
    &ocf::OCF,
    &bowtie::BOWTIE,
    &spiral::SPIRAL,
    &monopole::MONOPOLE,
    &groundplane::GROUNDPLANE,
    &fiveeighths::FIVE_EIGHTHS,
    &dipole::DIPOLE,
    &vdipole::VDIPOLE,
    &collinear::COLLINEAR,
    &jpole::JPOLE,
    &slimjim::SLIMJIM,
    &sleeve::SLEEVE,
    &discone::DISCONE,
    &discmonopole::DISCMONOPOLE,
    &coil::COIL,
    &lindenblad::LINDENBLAD,
    &turnstile::TURNSTILE,
    &qfh::QFH,
    &patch::PATCH,
    &ifa::IFA,
];

pub fn by_id(id: &str) -> &'static Design {
    DESIGNS.iter().copied().find(|d| d.id == id).unwrap_or(DESIGNS[0])
}
