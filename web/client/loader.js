const version = new URL(import.meta.url).searchParams.get("v") ?? "dev";
window.__DAINIKI_ASSET_VERSION__ = version;

const { default: init } = await import(`/pkg/dainiki_client.js?v=${version}`);
await init();
