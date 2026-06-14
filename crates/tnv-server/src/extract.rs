// src/extract.rs — request extractors.

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use crate::state::AppState;

/// The client's IP address, used as the rate-limiting key.
///
/// Behind a reverse proxy every request's TCP peer is the proxy, which would
/// collapse all clients into one bucket. When `TRUSTED_PROXY` is set we instead
/// read `X-Forwarded-For` and take the **rightmost** entry — the value appended
/// by our own proxy, which a client cannot spoof (any client-supplied XFF is to
/// the left of it). When not behind a proxy we use the real TCP peer and ignore
/// any `X-Forwarded-For` header (which would be attacker-controlled).
pub struct ClientIp(pub IpAddr);

impl FromRequestParts<AppState> for ClientIp {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ci| ci.0.ip())
            .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));

        if state.trusted_proxy {
            if let Some(ip) = parts
                .headers
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.rsplit(',').next())
                .map(str::trim)
                .and_then(|s| s.parse::<IpAddr>().ok())
            {
                return Ok(ClientIp(ip));
            }
        }
        Ok(ClientIp(peer))
    }
}
