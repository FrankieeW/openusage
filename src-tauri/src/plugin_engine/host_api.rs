use aes_gcm::{
    AesGcm, Nonce,
    aead::{Aead, KeyInit, OsRng, generic_array::typenum::U16, rand_core::RngCore},
    aes::Aes256,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use rquickjs::{Ctx, Exception, Function, Object, function::Rest};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

mod ccusage;
mod crypto;
mod deadline;
mod env;
mod env_overrides;
mod filesystem;
mod http;
mod keychain;
mod language_server;
mod logging;
mod redaction;
mod sqlite;
mod support;
mod utils;

use ccusage::inject_ccusage;
pub use ccusage::patch_ccusage_wrapper;
use crypto::inject_crypto;
pub(crate) use deadline::ProbeDeadline;
use deadline::{log_probe_deadline_skip, probe_timeout_error};
use env::{inject_env, read_env_from_process};
use env_overrides::resolve_env_for_plugin;
pub use env_overrides::{set_allow_all_env, set_env_overrides};
use filesystem::inject_fs;
use http::inject_http;
pub use http::patch_http_wrapper;
use keychain::inject_keychain;
use language_server::inject_ls;
pub use language_server::patch_ls_wrapper;
use logging::inject_log;
pub(crate) use redaction::redact_log_message;
use redaction::{redact_body, redact_url};
use sqlite::inject_sqlite;
use support::{expand_path, iso_now};
pub use utils::inject_utils;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvOverrideKind {
    Literal,
    Reference,
}

#[derive(Clone, Debug)]
pub struct EnvOverride {
    pub kind: EnvOverrideKind,
    pub value: String,
}

/// One entry as received from the frontend over IPC.
pub struct EnvOverrideInput {
    pub name: String,
    pub kind: EnvOverrideKind,
    pub value: String,
}

#[cfg(test)]
pub(crate) fn inject_host_api<'js>(
    ctx: &Ctx<'js>,
    plugin_id: &str,
    app_data_dir: &Path,
    app_version: &str,
) -> rquickjs::Result<()> {
    inject_host_api_with_deadline(
        ctx,
        plugin_id,
        app_data_dir,
        app_version,
        ProbeDeadline::none(),
    )
}

pub(crate) fn inject_host_api_with_deadline<'js>(
    ctx: &Ctx<'js>,
    plugin_id: &str,
    app_data_dir: &Path,
    app_version: &str,
    deadline: ProbeDeadline,
) -> rquickjs::Result<()> {
    let globals = ctx.globals();
    let probe_ctx = Object::new(ctx.clone())?;

    probe_ctx.set("nowIso", iso_now())?;

    let app_obj = Object::new(ctx.clone())?;
    app_obj.set("version", app_version)?;
    app_obj.set("platform", std::env::consts::OS)?;
    app_obj.set("appDataDir", app_data_dir.to_string_lossy().to_string())?;
    let plugin_data_dir = app_data_dir.join("plugins_data").join(plugin_id);
    if let Err(err) = std::fs::create_dir_all(&plugin_data_dir) {
        log::warn!(
            "[plugin:{}] failed to create plugin data dir: {}",
            plugin_id,
            err
        );
    }
    app_obj.set(
        "pluginDataDir",
        plugin_data_dir.to_string_lossy().to_string(),
    )?;
    probe_ctx.set("app", app_obj)?;

    let host = Object::new(ctx.clone())?;
    inject_log(ctx, &host, plugin_id)?;
    inject_fs(ctx, &host)?;
    inject_crypto(ctx, &host)?;
    inject_env(ctx, &host, plugin_id)?;
    inject_http(ctx, &host, plugin_id, deadline)?;
    inject_keychain(ctx, &host, plugin_id)?;
    inject_sqlite(ctx, &host)?;
    inject_ls(ctx, &host, plugin_id)?;
    inject_ccusage(ctx, &host, plugin_id, deadline)?;

    probe_ctx.set("host", host)?;
    globals.set("__openusage_ctx", probe_ctx)?;

    Ok(())
}

#[cfg(test)]
#[path = "host_api/tests/mod.rs"]
mod tests;
