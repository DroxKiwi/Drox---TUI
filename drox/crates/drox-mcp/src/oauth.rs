//! Réexport des primitives OAuth du SDK `rmcp`.
//!
//! Un flux OAuth complet (authorization code, refresh, stockage persistant)
//! reste à brancher côté `drox-cli` / extension VS Code. Pour les serveurs MCP
//! HTTP qui exigent déjà un jeton, passez `Authorization: Bearer …` dans
//! `McpRemoteSpec.headers` (avec expansion `${VAR}` dans la config).

pub use rmcp::transport::{
    AuthClient, AuthError, AuthorizationManager, AuthorizationSession, AuthorizedHttpClient,
    ClientCredentialsConfig, InMemoryCredentialStore, InMemoryStateStore, StoredCredentials,
    WWWAuthenticateParams,
};
