const path = require("node:path");

const DEFAULT_MANIFEST_PATH = path.join(__dirname, "manifest.json");

const DEFAULT_WEB_BASE_URL = "http://127.0.0.1:3100";


module.exports = { DEFAULT_MANIFEST_PATH, DEFAULT_WEB_BASE_URL };
