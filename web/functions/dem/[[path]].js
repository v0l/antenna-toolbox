const TILE = /^Copernicus_DSM_COG_10_[NS]\d{2}_00_[EW]\d{3}_00_DEM\/Copernicus_DSM_COG_10_[NS]\d{2}_00_[EW]\d{3}_00_DEM\.tif$/;

export async function onRequestGet({ params }) {
  const path = Array.isArray(params.path) ? params.path.join("/") : params.path;
  if (!TILE.test(path)) {
    return new Response("not a Copernicus GLO-30 tile", { status: 404 });
  }
  const upstream = await fetch(`https://copernicus-dem-30m.s3.amazonaws.com/${path}`, {
    cf: { cacheTtl: 2592000, cacheEverything: true },
  });
  const headers = new Headers();
  headers.set("Content-Type", "image/tiff");
  headers.set("Cache-Control", "public, max-age=2592000, immutable");
  const length = upstream.headers.get("Content-Length");
  if (length) headers.set("Content-Length", length);
  return new Response(upstream.body, { status: upstream.status, headers });
}
