# Connect an MCP client with OAuth

[中文](README.md)

Copy the server URL from **Settings → MCP → Connect client → OAuth**. The existing ChatGPT tab remains available, and the Common tab and other client configurations keep their existing behavior.

1flowbase supports HTTPS callbacks, the OAuth authorization code flow, PKCE S256, and dynamic client registration (DCR). Token endpoint authentication is `none`: the client ID is registered automatically and no client secret is required. Enter your user API key only on the 1flowbase authorization page, never in a client secret field or server URL.

## Connect from Gemini on the web

1. Open **Settings → Connected Apps** in Gemini and add a custom app.
2. Paste the complete server URL from the OAuth tab, including its `origin` parameter.
3. Leave **Client ID / Client secret blank** and click **Next**.
4. On the 1flowbase authorization page, verify a user API key. Check the client callback domain, workspace, and MCP instance before authorizing.
5. Return to Gemini, type `@` to select the app, and invoke a tool you have permission to use. Creating the connection alone does not verify tool execution.

You do not need to save an API key in the connection dialog. Access tokens refresh automatically for up to 30 days per authorization. Key expiration or revocation, permission changes, or disabling the MCP instance invalidate the connection and require authorization again.

Check [Google's custom app requirements](https://support.google.com/gemini/answer/17209137) for account and regional availability. Use [Gemini Developer Mode](https://developers.google.com/connected-apps/mcp/dev-mode) to inspect tool schemas and troubleshoot execution.

## Deployment checks

The public HTTPS origin must serve `/api/` and `/mcp/authorize`. Send an unauthenticated MCP request, follow the resource metadata URL in its `WWW-Authenticate: Bearer resource_metadata="…"` header, then read the issuer from `authorization_servers`.

Both endpoints below must return HTTP 200 and matching OAuth metadata, including `registration_endpoint`, authorization code support, and PKCE capabilities:

```text
<issuer>/.well-known/oauth-authorization-server
<issuer>/.well-known/openid-configuration
```

Keep the full generated issuer, including any `/origins/…` context. Google may stop discovery if the first endpoint returns an error, even if the second works. If a reverse proxy intercepts root `/.well-known/` paths, ensure that the `/api/` discovery endpoints above remain accessible. See [Google's OAuth guide](https://developers.google.com/connected-apps/mcp/oauth).

DCR accepts eligible HTTPS callbacks without restricting registration by client brand. Authorization requests must exactly match a registered callback. Client names are self-reported, so the authorization page also displays the callback origin; do not trust a client based on its name alone. The server never fetches callback URLs.

The [ChatGPT connection guide](../chatgpt-mcp.en.md) still applies. Existing ChatGPT client identities and refresh-token rotation remain compatible.

Generic OAuth clients reuse their refresh token within the existing grant. Refresh does not extend the 30-day authorization limit. ChatGPT retains its existing rotation policy.

OAuth metadata advertises a `revocation_endpoint`. Clients can submit an access or refresh token when disconnecting to revoke the entire authorization.
