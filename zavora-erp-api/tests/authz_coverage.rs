//! Authorization-coverage guarantee (RBAC v2).
//!
//! The declarative `ROUTE_PERMISSIONS` registry (middleware/route_perms.rs) is the
//! single, auditable access-control matrix, enforced centrally with default-deny.
//! This test proves it is COMPLETE and VALID by static analysis:
//!   (a) every protected route registered in `main.rs` has a registry entry;
//!   (b) every registry permission key is a real catalog permission;
//!   (c) no registry entry is an orphan (points at a route that doesn't exist).
//! CI fails on any gap — so a new route cannot ship without an explicit, audited
//! permission, and a typo'd/renamed permission cannot slip through.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use zavora_erp_core::rbac::permission_catalog;

fn crate_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

const METHODS: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE"];

/// Parse `ROUTE_PERMISSIONS` rows from route_perms.rs → ((method, path), perm_key?).
fn parse_registry() -> Vec<((String, String), Option<String>)> {
    let src = fs::read_to_string(crate_path("src/middleware/route_perms.rs")).expect("read route_perms.rs");
    let mut out = Vec::new();
    for line in src.lines() {
        let l = line.trim();
        if !l.starts_with("(\"") {
            continue;
        }
        // ("GET", "/api/v1/x", Access::Perm("a.b")),  |  ("GET", "/x", Access::SelfScoped),
        let parts: Vec<&str> = l.splitn(3, ',').collect();
        if parts.len() < 3 {
            continue;
        }
        let method = parts[0].trim().trim_start_matches('(').trim().trim_matches('"').to_string();
        let path = parts[1].trim().trim_matches('"').to_string();
        let perm = if let Some(i) = parts[2].find("Access::Perm(\"") {
            let rest = &parts[2][i + "Access::Perm(\"".len()..];
            Some(rest[..rest.find('"').unwrap()].to_string())
        } else {
            None // SelfScoped
        };
        out.push(((method, path), perm));
    }
    out
}

/// Extract (method, path) for every route registered in the protected router of main.rs.
fn protected_routes() -> BTreeSet<(String, String)> {
    let src = fs::read_to_string(crate_path("src/main.rs")).expect("read main.rs");
    let start = src.find("let protected = Router::new()").expect("protected router");
    let after = &src[start..];
    let end = after.find(".route_layer").unwrap_or(after.len());
    let block = &after[..end];
    let mut out = BTreeSet::new();
    for line in block.lines() {
        let l = line.trim();
        if !l.starts_with(".route(\"") {
            continue;
        }
        // .route("PATH", get(..).post(..))
        let path = {
            let rest = &l[".route(\"".len()..];
            rest[..rest.find('"').unwrap()].to_string()
        };
        let body = &l[l.find(',').map(|i| i + 1).unwrap_or(0)..];
        for m in METHODS {
            let lower = m.to_lowercase();
            if body.contains(&format!("{lower}(")) {
                out.insert((m.to_string(), path.clone()));
            }
        }
    }
    out
}

#[test]
fn registry_covers_every_protected_route_with_valid_permissions() {
    let registry = parse_registry();
    let reg_set: BTreeSet<(String, String)> = registry.iter().map(|(k, _)| k.clone()).collect();
    let routes = protected_routes();

    assert!(routes.len() > 200, "sanity: expected >200 protected routes, found {}", routes.len());
    assert!(registry.len() > 200, "sanity: expected >200 registry rows, found {}", registry.len());

    // (a) Every protected route has a registry entry.
    let missing: Vec<String> = routes
        .iter()
        .filter(|r| !reg_set.contains(*r))
        .map(|(m, p)| format!("{m} {p}"))
        .collect();
    assert!(missing.is_empty(), "protected routes with NO permission mapping (default-deny gap):\n  {}", missing.join("\n  "));

    // (b) Every registry permission key exists in the catalog.
    let catalog: BTreeSet<String> = permission_catalog().into_iter().map(|p| p.key).collect();
    let bad: Vec<String> = registry
        .iter()
        .filter_map(|((m, p), perm)| perm.as_ref().filter(|k| !catalog.contains(*k)).map(|k| format!("{m} {p} -> {k}")))
        .collect();
    assert!(bad.is_empty(), "registry entries referencing unknown permission keys:\n  {}", bad.join("\n  "));

    // (c) No orphan registry entries (every entry maps to a real route).
    let orphans: Vec<String> = reg_set
        .iter()
        .filter(|r| !routes.contains(*r))
        .map(|(m, p)| format!("{m} {p}"))
        .collect();
    assert!(orphans.is_empty(), "registry entries with no matching route (stale):\n  {}", orphans.join("\n  "));
}
