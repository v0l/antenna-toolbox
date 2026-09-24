# Browser build

```sh
web/build.sh             # wasm-pack build into web/pkg
python3 web/serve.py     # serves web/ on 127.0.0.1:8080, with /dem proxied to Copernicus
web/deploy.sh            # builds and deploys to wire-antenna-calc.pages.dev
```

The Copernicus GLO-30 bucket sends no CORS headers, so the browser fetches terrain
tiles from `dem/` on the same origin. Whatever serves the page must proxy
`/dem/` to `https://copernicus-dem-30m.s3.amazonaws.com/`, as `serve.py` does, or
the page can be opened with `?dem=<url>` pointing at another mirror. On Cloudflare
Pages, `functions/dem/[[path]].js` is that proxy; it only passes GLO-30 tile paths.

The browser build solves on one CPU thread and has no GPU fill, so large coverage
maps take about a minute. The VNA tab talks to a NanoVNA-H or H4 through Web
Serial, which Chrome and Edge support.
