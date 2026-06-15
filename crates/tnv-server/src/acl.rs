// src/acl.rs — authorization gate for /api/admin/** built on axum-acl.
//
// Authentication (token validity) stays ours: the role extractor cryptographically
// verifies the JWT. axum-acl only decides allow/deny from the rule table. We do
// NOT use the default X-Roles header extractor (which is client-settable).

use axum::response::{IntoResponse, Response};
use axum::http::{Request, StatusCode};
use axum::Json;
use serde_json::json;
use axum_acl::{
    AccessDenied, AccessDeniedHandler, AclAction, AclLayer, AclRuleFilter, AclTable,
    HeaderIdExtractor, RoleExtractionResult, RoleExtractor,
};

use crate::auth::verify_jwt;
use crate::models::Claims;

/// Role bits.
pub const ROLE_ADMIN: u32 = 1 << 0;
pub const ROLE_USER: u32 = 1 << 1;

/// Extracts roles by verifying the bearer JWT and mapping it to a role bitmask.
#[derive(Clone)]
pub struct JwtRoleExtractor {
    secret: String,
}

impl JwtRoleExtractor {
    pub fn new(secret: String) -> Self {
        Self { secret }
    }

    fn claims_from<B>(&self, request: &Request<B>) -> Option<Claims> {
        let token = request
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))?;
        verify_jwt(token, &self.secret).ok()
    }
}

impl<B> RoleExtractor<B> for JwtRoleExtractor {
    fn extract_roles(&self, request: &Request<B>) -> RoleExtractionResult {
        match self.claims_from(request) {
            Some(claims) if claims.is_admin() => {
                RoleExtractionResult::Roles(ROLE_ADMIN | ROLE_USER)
            }
            Some(_) => RoleExtractionResult::Roles(ROLE_USER),
            None => RoleExtractionResult::Anonymous,
        }
    }
}

/// JSON 403 for denied admin requests (matches the rest of the API).
#[derive(Clone)]
struct JsonDenied;

impl AccessDeniedHandler for JsonDenied {
    fn handle(&self, _denied: &AccessDenied) -> Response {
        (StatusCode::FORBIDDEN, Json(json!({"error": "admin access required"}))).into_response()
    }
}

/// Build the ACL layer that gates the admin routes to admin-tier callers.
///
/// This layer is applied only to the admin sub-router, and axum strips the
/// `/api/admin` nest prefix before the layer runs (it would see `/users`, not
/// `/api/admin/users`). So the rule matches ANY path reaching this layer and
/// simply requires the admin role; everything non-admin hits the default Deny.
pub fn admin_layer(jwt_secret: String) -> AclLayer<JwtRoleExtractor, HeaderIdExtractor> {
    let table = AclTable::builder()
        .default_action(AclAction::Deny)
        .add_any(
            AclRuleFilter::new()
                .role_mask(ROLE_ADMIN)
                .action(AclAction::Allow),
        )
        .build();

    AclLayer::new(table)
        .with_role_extractor(JwtRoleExtractor::new(jwt_secret))
        .with_denied_handler(JsonDenied)
}
