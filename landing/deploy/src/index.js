const APP_URL = "https://app.dainiki.smbl.dev/";

export default {
  async fetch(request, env) {
    const url = new URL(request.url);

    if (url.pathname === "/app" || url.pathname === "/app/") {
      return Response.redirect(APP_URL, 302);
    }

    if (url.pathname === "/") {
      return env.ASSETS.fetch(
        new Request(new URL("/index.html", url.origin), request),
      );
    }

    return env.ASSETS.fetch(request);
  },
};
