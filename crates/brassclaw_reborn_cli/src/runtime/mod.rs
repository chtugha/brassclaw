use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::time::Duration;
use std::{future::Future, thread};

use anyhow::Context;

use brassclaw_host_api::runtime_policy::RuntimeProfile;
use brassclaw_reborn_composition::{
    OAuthClientConfig, PollSettings, RebornBuildInput, RebornCompositionProfile,
    RebornLocalRuntimeProfileOptions, RebornRuntimeIdentity, RebornRuntimeInput,
    TurnRunnerSettings, build_reborn_runtime, local_runtime_build_input_with_options,
};
use brassclaw_reborn_config::RebornBootConfig;
use secrecy::SecretString;
use tokio_util::sync::CancellationToken;

use crate::context::RebornCliContext;

/// Environment variable for the new fine-grained runtime profile knob.
pub(crate) const RUNTIME_PROFILE_ENV: &str = "BRASSCLAW_RUNTIME_PROFILE";

/// Legacy environment variable — deprecated in Phase 11. Use `RUNTIME_PROFILE_ENV`.
const REBORN_PROFILE_ENV: &str = "BRASSCLAW_REBORN_PROFILE";

#[cfg(test)]
mod test_env;
mod trigger_poller;

use trigger_poller::trigger_poller_settings;

pub(crate) fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::fmt;
    let filter = EnvFilter::try_from_env("BRASSCLAW_REBORN_LOG").unwrap_or_else(|_| {
        EnvFilter::new("info,brassclaw_reborn=info,brassclaw_reborn_composition=info")
    });
    let _ = fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

pub(crate) fn block_on_cli<F, T, E>(future: F) -> anyhow::Result<T>
where
    F: Future<Output = Result<T, E>> + Send + 'static,
    T: Send + 'static,
    E: Into<anyhow::Error> + Send + 'static,
{
    if tokio::runtime::Handle::try_current().is_ok() {
        return thread::spawn(move || block_on_cli_future(future))
            .join()
            .map_err(|_| anyhow::anyhow!("CLI async task thread panicked"))?;
    }
    block_on_cli_future(future)
}

fn block_on_cli_future<F, T, E>(future: F) -> anyhow::Result<T>
where
    F: Future<Output = Result<T, E>>,
    E: Into<anyhow::Error>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(future).map_err(Into::into)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct RuntimeInputOptions {
    pub(crate) confirm_host_access: bool,
}

pub(crate) fn execute(
    context: RebornCliContext,
    message: Option<String>,
    options: RuntimeInputOptions,
) -> anyhow::Result<()> {
    let runtime_input =
        build_runtime_input_with_options(context.boot_config(), RuntimeInputCaller::Run, options)?;
    let boot_config = context.boot_config().clone();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(async move {
        let runtime_input =
            with_run_local_trigger_fire_access_checker(runtime_input, &boot_config).await?;
        let runtime = build_reborn_runtime(runtime_input).await?;
        print_runtime_banner(&boot_config);

        let conversation = runtime.new_conversation().await?;
        let cancellation = install_ctrl_c_cancellation();

        let outcome = if let Some(text) = message {
            send_once(&runtime, &conversation, &text, cancellation).await
        } else {
            run_repl_loop(&runtime, &conversation, cancellation).await
        };

        runtime.shutdown().await?;
        outcome
    })?;
    Ok(())
}

/// Wires the local trigger-fire access checker into `runtime_input` for the
/// `run` command. Currently a no-op until embedded PG is plumbed into the
/// local-dev run path (TODO: wire pool after embedded-PG startup).
async fn with_run_local_trigger_fire_access_checker(
    runtime_input: RebornRuntimeInput,
    _config: &RebornBootConfig,
) -> anyhow::Result<RebornRuntimeInput> {
    Ok(runtime_input)
}

fn print_runtime_banner(config: &RebornBootConfig) {
    eprintln!("brassclaw-reborn: runtime started");
    eprintln!("  reborn_home : {}", config.home().path().display());
    eprintln!();
}

async fn send_once(
    runtime: &brassclaw_reborn_composition::RebornRuntime,
    conversation: &brassclaw_reborn_composition::ConversationId,
    text: &str,
    cancellation: CancellationToken,
) -> anyhow::Result<()> {
    let reply = runtime
        .send_user_message_with_cancellation(conversation, text, cancellation)
        .await?;
    if !reply.is_successful_final_reply() {
        anyhow::bail!(
            "reborn run did not produce an assistant reply (status={:?}, run_id={})",
            reply.status,
            reply.run_id
        );
    }
    print_reply(&reply);
    Ok(())
}

async fn run_repl_loop(
    runtime: &brassclaw_reborn_composition::RebornRuntime,
    conversation: &brassclaw_reborn_composition::ConversationId,
    cancellation: CancellationToken,
) -> anyhow::Result<()> {
    let stdin_is_tty = std::io::stdin().is_terminal();
    if stdin_is_tty {
        eprintln!("(repl) type a message and press enter; Ctrl-D to exit");
    }
    let stdin = tokio::io::stdin();
    let reader = tokio::io::BufReader::new(stdin);
    use tokio::io::AsyncBufReadExt;
    let mut lines = reader.lines();

    loop {
        if stdin_is_tty {
            // Prompt to stderr so stdout stays clean for piping.
            eprint!("> ");
            let _ = std::io::stderr().flush();
        }
        tokio::select! {
            line = lines.next_line() => {
                match line? {
                    Some(text) if text.trim().is_empty() => continue,
                    Some(text) if is_exit_command(&text) => return Ok(()),
                    Some(text) if is_help_command(&text) => {
                        print_repl_help();
                        continue;
                    }
                    Some(text) => {
                        match runtime
                            .send_user_message_with_cancellation(
                                conversation,
                                &text,
                                cancellation.clone(),
                            )
                            .await
                        {
                            Ok(reply) if reply.is_successful_final_reply() => print_reply(&reply),
                            Ok(reply) if stdin_is_tty => print_reply(&reply),
                            Ok(reply) => {
                                anyhow::bail!(
                                    "reborn run did not produce an assistant reply (status={:?}, run_id={})",
                                    reply.status,
                                    reply.run_id
                                );
                            }
                            Err(error) if stdin_is_tty => {
                                eprintln!("error: {error}");
                                if cancellation.is_cancelled() {
                                    return Ok(());
                                }
                            }
                            Err(error) => return Err(error.into()),
                        }
                    }
                    None => {
                        if stdin_is_tty {
                            eprintln!();
                        }
                        return Ok(());
                    }
                }
            }
            _ = cancellation.cancelled() => {
                eprintln!();
                eprintln!("(repl) caught ctrl-c, shutting down");
                return Ok(());
            }
        }
    }
}

fn is_exit_command(text: &str) -> bool {
    matches!(text.trim(), "/exit" | "/quit")
}

fn is_help_command(text: &str) -> bool {
    text.trim() == "/help"
}

fn print_repl_help() {
    eprintln!("Reborn REPL commands:");
    eprintln!("  /help  Show this help");
    eprintln!("  /exit  Exit the REPL");
    eprintln!("  /quit  Exit the REPL");
}

fn print_reply(reply: &brassclaw_reborn_composition::AssistantReply) {
    match reply.text.as_deref() {
        Some(text) => println!("{text}"),
        None => eprintln!(
            "(no assistant text; status={:?}, run_id={})",
            reply.status, reply.run_id
        ),
    }
}

fn install_ctrl_c_cancellation() -> CancellationToken {
    let cancellation = CancellationToken::new();
    let ctrl_c_cancellation = cancellation.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            ctrl_c_cancellation.cancel();
        }
    });
    cancellation
}

/// Which subcommand is asking for the runtime input. Used to decide
/// which `[identity]` / `[…]` config sections are legitimate vs.
/// "parsed but not wired" — the runtime slice today does not honor
/// `[identity].default_project`, but the `serve` subcommand stamps it
/// onto every authenticated WebUI caller and therefore consumes it
/// directly. Without this discriminator the shared `build_runtime_input`
/// would reject `serve` configs that legitimately set
/// `default_project`. See the `reject_unsupported_runtime_sections`
/// branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeInputCaller {
    Run,
    Serve,
}

#[cfg(test)]
pub(crate) fn build_runtime_input(
    config: &RebornBootConfig,
    caller: RuntimeInputCaller,
) -> anyhow::Result<RebornRuntimeInput> {
    build_runtime_input_with_options(config, caller, RuntimeInputOptions::default())
}

pub(crate) fn build_runtime_input_with_options(
    config: &RebornBootConfig,
    caller: RuntimeInputCaller,
    options: RuntimeInputOptions,
) -> anyhow::Result<RebornRuntimeInput> {
    let runtime_services = build_services_input_with_options(config, caller, options)?;

    // Behavior flags that have no per-provider equivalent are still read from
    // the config file. Token budget fields (conversation_history, skills,
    // identity, capability_surface) are now resolved from the per-provider DB
    // at runtime startup in build_reborn_runtime; no file-config fallback is
    // passed — the compiled defaults apply when the DB has no row.
    let behavior_flags = behavior_flags_from_config(runtime_services.config_file.as_ref());

    #[allow(unused_mut)]
    let mut runtime_input = RebornRuntimeInput::from_services(runtime_services.services_input)
        .with_runner_settings(runner_settings(runtime_services.config_file.as_ref())?)
        .with_trigger_poller_settings(trigger_poller_settings(
            runtime_services.config_file.as_ref(),
        )?)
        .with_poll_settings(PollSettings {
            interval: Duration::from_millis(200),
            max_total: Duration::from_secs(180),
        })
        .with_identity(runtime_identity(runtime_services.config_file.as_ref()))
        .with_regex_skill_activation_enabled(regex_skill_activation_enabled(
            runtime_services.config_file.as_ref(),
        ))
        .with_capability_focus_enabled(behavior_flags.capability_focus_enabled)
        .with_planning_mode_enabled(behavior_flags.planning_mode_enabled)
        .with_content_cache_threshold(behavior_flags.content_cache_threshold)
        .with_plan_library_enabled(behavior_flags.plan_library_enabled)
        .with_skill_promotion_threshold(behavior_flags.skill_promotion_threshold);

    #[cfg(feature = "root-llm-provider")]
    {
        match brassclaw_reborn_composition::resolve_reborn_runtime_llm(
            config,
            runtime_services.config_file.as_ref(),
        )? {
            Some(llm) => {
                tracing::debug!(
                    provider_id = %llm.provider_id(),
                    model = %llm.model(),
                    "resolved LLM selection for Reborn runtime"
                );
                runtime_input = runtime_input.with_resolved_llm(llm);
            }
            None => {
                tracing::warn!(
                    "no LLM selection configured; set `[llm.default]` in config.toml or configure \
                     LLM_BACKEND / provider environment variables. Runs will fail until an \
                     LLM is wired."
                );
            }
        }
        // Carry the boot config so the WebUI facade can compose the operator
        // LLM-config settings service over `providers.json` / `config.toml`.
        runtime_input = runtime_input.with_boot_config(config.clone());
    }

    Ok(runtime_input)
}

pub(crate) struct RuntimeServicesInput {
    pub(crate) services_input: RebornBuildInput,
    config_file: Option<brassclaw_reborn_config::RebornConfigFile>,
}

#[derive(Clone, Debug)]
pub(crate) struct ResolvedGoogleOAuthConfig {
    pub(crate) client: OAuthClientConfig,
    pub(crate) hosted_domain_hint: Option<String>,
}

pub(crate) fn build_services_input_with_options(
    config: &RebornBootConfig,
    caller: RuntimeInputCaller,
    options: RuntimeInputOptions,
) -> anyhow::Result<RuntimeServicesInput> {
    // Read the operator's boot TOML if present. Missing file is OK
    // (operator may not have run `brassclaw-reborn config init` yet);
    // sparse fields are OK (each absent field falls back to the
    // CLI-shaped default baked into composition).
    let config_file = read_config_file(config)?;

    reject_unsupported_runtime_sections(config_file.as_ref(), caller)?;

    let owner_id = default_owner_id(config_file.as_ref());

    let local_dev_root: PathBuf = config.home().path().join("local-dev");

    // Use a safe default workspace location to avoid overlap with skill storage.
    // Allow override via environment variable for advanced use cases.
    let workspace_root = if let Ok(custom_workspace) = std::env::var("BRASSCLAW_WORKSPACE_ROOT") {
        PathBuf::from(custom_workspace)
    } else {
        local_dev_root.join("workspace")
    };
    // BRASSCLAW_RUNTIME_PROFILE wins over legacy BRASSCLAW_REBORN_PROFILE.
    // For non-local RuntimeProfile values the fail-closed guard in
    // runtime_profile_from_env() will already have errored out, so by the
    // time we reach here we know the effective profile is local.
    let composition = if let Some(rt_profile) = runtime_profile_from_env()? {
        // Translate the fine-grained RuntimeProfile to the coarse composition
        // profile — only local variants are reachable here (fail-closed guard
        // above already rejected non-local profiles without PG_URL).
        match rt_profile {
            RuntimeProfile::LocalYolo => RebornCompositionProfile::LocalDevYolo,
            _ => RebornCompositionProfile::LocalDev,
        }
    } else {
        composition_profile_from_legacy_env(config, config_file.as_ref())?
    };
    let mut services_input = local_runtime_build_input_with_options(
        composition,
        owner_id,
        local_dev_root,
        RebornLocalRuntimeProfileOptions {
            confirm_host_access: options.confirm_host_access,
        },
    )
    .with_context(
        || "brassclaw-reborn run currently supports profile=local-dev or profile=local-dev-yolo",
    )?
    .with_local_dev_workspace_root(workspace_root);
    if services_input.requires_local_dev_confirmed_host_home_root() {
        let host_home_root =
            confirmed_host_home_root(options).context("local-dev-yolo host access")?;
        services_input = services_input.with_local_dev_confirmed_host_home_root(host_home_root);
    }
    if let Some(ResolvedGoogleOAuthConfig {
        client,
        hosted_domain_hint: _hosted_domain_hint,
    }) = resolve_google_oauth_config_from_env()?
    {
        services_input = services_input.with_google_oauth_backend(client);
    }

    Ok(RuntimeServicesInput {
        services_input,
        config_file,
    })
}

pub(crate) fn resolve_google_oauth_config_from_env()
-> anyhow::Result<Option<ResolvedGoogleOAuthConfig>> {
    resolve_google_oauth_config(optional_nonempty_env)
}

fn resolve_google_oauth_config(
    mut lookup: impl FnMut(&str) -> Option<String>,
) -> anyhow::Result<Option<ResolvedGoogleOAuthConfig>> {
    let reborn_client_id = lookup("BRASSCLAW_REBORN_GOOGLE_CLIENT_ID");
    let reborn_redirect_uri = lookup("BRASSCLAW_REBORN_GOOGLE_OAUTH_REDIRECT_URI");
    let reborn_client_secret = lookup("BRASSCLAW_REBORN_GOOGLE_CLIENT_SECRET");
    let reborn_hosted_domain_hint = lookup("BRASSCLAW_REBORN_GOOGLE_HOSTED_DOMAIN_HINT");
    let legacy_client_id = lookup("GOOGLE_CLIENT_ID");
    let legacy_client_secret = lookup("GOOGLE_CLIENT_SECRET");
    let legacy_redirect_uri = lookup("GOOGLE_OAUTH_REDIRECT_URI");
    let legacy_hosted_domain_hint = lookup("GOOGLE_ALLOWED_HD");

    if reborn_client_id.is_none()
        && reborn_redirect_uri.is_none()
        && reborn_client_secret.is_none()
        && reborn_hosted_domain_hint.is_none()
        && legacy_client_id.is_none()
        && legacy_client_secret.is_none()
        && legacy_redirect_uri.is_none()
        && legacy_hosted_domain_hint.is_none()
    {
        return Ok(None);
    }

    let client_id = reborn_client_id
        .or(legacy_client_id)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "BRASSCLAW_REBORN_GOOGLE_CLIENT_ID or GOOGLE_CLIENT_ID is required for Google OAuth setup"
            )
        })?;
    let redirect_uri = reborn_redirect_uri.or(legacy_redirect_uri).ok_or_else(|| {
        anyhow::anyhow!(
            "BRASSCLAW_REBORN_GOOGLE_OAUTH_REDIRECT_URI or GOOGLE_OAUTH_REDIRECT_URI is required for Google OAuth setup"
        )
    })?;
    let client_secret = reborn_client_secret
        .or(legacy_client_secret)
        .map(SecretString::from);
    if client_secret.is_none() {
        tracing::warn!(
            target = "brassclaw::reborn::cli::google_oauth",
            "Google OAuth setup config has no client secret; token exchange will use public-client PKCE",
        );
    }
    let hosted_domain_hint = reborn_hosted_domain_hint.or(legacy_hosted_domain_hint);
    let mut client = OAuthClientConfig::new(client_id, redirect_uri, client_secret)
        .context("invalid Google OAuth client configuration")?;
    if let Some(hosted_domain_hint) = hosted_domain_hint.clone() {
        client = client.with_hosted_domain_hint(hosted_domain_hint);
    }

    Ok(Some(ResolvedGoogleOAuthConfig {
        client,
        hosted_domain_hint,
    }))
}

/// Read an env var with lenient presence semantics: unset OR
/// present-but-blank both collapse to `None`. Used for optional-config
/// callers (OAuth client overrides, etc.) where a blank slot is benign.
///
/// **Not** for operator-control knobs like `BRASSCLAW_TRIGGER_POLLER_*` —
/// those use a strict-presence variant in the `trigger_poller` submodule,
/// which treats a present-but-blank value as a fatal misconfiguration.
fn optional_nonempty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn default_owner_id(
    config_file: Option<&brassclaw_reborn_config::RebornConfigFile>,
) -> &str {
    config_file
        .and_then(|file| file.identity.as_ref())
        .and_then(|identity| identity.default_owner.as_deref())
        .unwrap_or("reborn-cli")
}

fn confirmed_host_home_root(options: RuntimeInputOptions) -> anyhow::Result<PathBuf> {
    debug_assert!(options.confirm_host_access);
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .context("HOME or USERPROFILE must be set")
}

pub(crate) fn read_config_file(
    config: &RebornBootConfig,
) -> anyhow::Result<Option<brassclaw_reborn_config::RebornConfigFile>> {
    use brassclaw_reborn_config::RebornConfigFile;
    let path = config.home().path().join("config.toml");
    let file = RebornConfigFile::load(&path).map_err(anyhow::Error::from)?;
    if let Some(parsed) = &file {
        tracing::debug!(
            path = %path.display(),
            api_version = ?parsed.api_version,
            "loaded boot config TOML"
        );
    }
    Ok(file)
}

// CLI-local operator config only. Product/WebUI identity must come from
// trusted host installation/binding resolution, not inbound payloads.
fn runtime_identity(
    config_file: Option<&brassclaw_reborn_config::RebornConfigFile>,
) -> RebornRuntimeIdentity {
    let default = RebornRuntimeIdentity::reborn_cli();
    let Some(identity) = config_file.and_then(|file| file.identity.as_ref()) else {
        return default;
    };

    RebornRuntimeIdentity {
        tenant_id: identity
            .tenant
            .clone()
            .unwrap_or_else(|| default.tenant_id.clone()),
        agent_id: identity
            .default_agent
            .clone()
            .unwrap_or_else(|| default.agent_id.clone()),
        source_binding_id: default.source_binding_id,
        reply_target_binding_id: default.reply_target_binding_id,
    }
}

fn regex_skill_activation_enabled(
    config_file: Option<&brassclaw_reborn_config::RebornConfigFile>,
) -> bool {
    config_file
        .and_then(|file| file.skills.as_ref())
        .and_then(|skills| skills.regex_activation_enabled)
        .unwrap_or(true)
}

/// Resolve the new `BRASSCLAW_RUNTIME_PROFILE` env var.
///
/// Returns `None` when the variable is absent (caller falls back to the local
/// composition profile mapping).  Emits a fail-closed error when:
/// - The value cannot be parsed as a `RuntimeProfile` wire name.
/// - A non-local profile is set but `BRASSCLAW_PG_URL` is unset.
pub(crate) fn runtime_profile_from_env() -> anyhow::Result<Option<RuntimeProfile>> {
    let Some(raw) = std::env::var_os(RUNTIME_PROFILE_ENV) else {
        return Ok(None);
    };
    let raw = raw.to_string_lossy();
    let profile: RuntimeProfile = raw.parse().map_err(|_| {
        anyhow::anyhow!(
            "{RUNTIME_PROFILE_ENV}={raw} is not a recognised runtime profile. \
             Valid values: secure_default, local_safe, local_dev, local_yolo, \
             hosted_safe, hosted_dev, hosted_yolo_tenant_scoped, \
             enterprise_safe, enterprise_dev, enterprise_yolo_dedicated, \
             sandboxed, experiment"
        )
    })?;
    // Fail-closed guard: non-local profiles require an external Postgres URL.
    if !profile.is_local() && std::env::var_os("BRASSCLAW_PG_URL").is_none() {
        anyhow::bail!(
            "Non-local runtime profile '{profile}' requires BRASSCLAW_PG_URL. \
             Embedded Postgres is for single-host local deployments only. \
             Set BRASSCLAW_PG_URL to an external Postgres URL or use a local \
             runtime profile (local_dev, local_safe, local_yolo)."
        );
    }
    Ok(Some(profile))
}

/// Emit a deprecation warning when `BRASSCLAW_REBORN_PROFILE` is set and
/// translate its value to the equivalent local composition profile.
///
/// When `BRASSCLAW_RUNTIME_PROFILE` is also set it wins; this function is
/// only invoked for the old-var path.  Parses the raw env string directly —
/// `RebornProfile` and `config.profile()` were removed in Phase 11.
///
/// Returns the `RebornCompositionProfile` to use for this boot.
pub(crate) fn composition_profile_from_legacy_env(
    _config: &RebornBootConfig,
    _config_file: Option<&brassclaw_reborn_config::RebornConfigFile>,
) -> anyhow::Result<RebornCompositionProfile> {
    let Some(raw_os) = std::env::var_os(REBORN_PROFILE_ENV) else {
        // No legacy var set — default to local-dev.
        return Ok(RebornCompositionProfile::LocalDev);
    };

    let raw = raw_os.to_string_lossy();
    eprintln!(
        "WARNING: BRASSCLAW_REBORN_PROFILE is deprecated. \
         Use BRASSCLAW_RUNTIME_PROFILE instead. \
         See 'brassclaw runtime-profile list' for available values."
    );

    match raw.as_ref() {
        "local-dev" => Ok(RebornCompositionProfile::LocalDev),
        "local-dev-yolo" => Ok(RebornCompositionProfile::LocalDevYolo),
        "production" => {
            eprintln!(
                "WARNING: BRASSCLAW_REBORN_PROFILE=production is deprecated and no longer \
                 implies a security policy. Defaulting to BRASSCLAW_RUNTIME_PROFILE=local_dev. \
                 Set BRASSCLAW_RUNTIME_PROFILE explicitly for your deployment tier."
            );
            Ok(RebornCompositionProfile::LocalDev)
        }
        "migration-dry-run" => {
            anyhow::bail!(
                "BRASSCLAW_REBORN_PROFILE=migration-dry-run is removed. \
                 Use 'brassclaw migrate --dry-run' instead."
            )
        }
        other => {
            anyhow::bail!(
                "BRASSCLAW_REBORN_PROFILE={other} is not a recognised profile value. \
                 Use BRASSCLAW_RUNTIME_PROFILE with one of: local_dev, local_safe, local_yolo, \
                 hosted_safe, etc. (see 'brassclaw runtime-profile list')."
            )
        }
    }
}

fn reject_unsupported_runtime_sections(
    config_file: Option<&brassclaw_reborn_config::RebornConfigFile>,
    caller: RuntimeInputCaller,
) -> anyhow::Result<()> {
    let Some(file) = config_file else {
        return Ok(());
    };

    // `[identity].default_project` is parsed but not yet wired into the
    // generic runtime slice — `run` / `repl` would silently drop the value,
    // so we fail-loud. The `serve` subcommand DOES consume it (stamped onto
    // every `WebUiAuthenticatedCaller`), so for that caller the field is
    // supported, not "parsed but not wired".
    if let Some(identity) = file.identity.as_ref()
        && identity.default_project.is_some()
        && caller != RuntimeInputCaller::Serve
    {
        anyhow::bail!(
            "config file [identity] field default_project is parsed but not wired in this runtime slice; \
             leave it commented until project-scope wiring lands"
        );
    }

    let mut sections = Vec::new();
    if file.policy.is_some() {
        sections.push("[policy]");
    }
    if file.drivers.is_some() {
        sections.push("[drivers]");
    }
    if file.harness.is_some() {
        sections.push("[harness]");
    }
    if sections.is_empty() {
        Ok(())
    } else {
        anyhow::bail!(
            "config file section(s) {} are parsed but not wired in this runtime slice; \
             leave them commented until epic #3036 substrate lands",
            sections.join(", ")
        )
    }
}

fn behavior_flags_from_config(
    config_file: Option<&brassclaw_reborn_config::RebornConfigFile>,
) -> brassclaw_reborn_config::ResolvedTokenBudgets {
    let Some(tokens) = config_file.and_then(|file| file.tokens.as_ref()) else {
        return brassclaw_reborn_config::ResolvedTokenBudgets::default();
    };
    brassclaw_reborn_config::resolve_with_profile(tokens)
}

fn runner_settings(
    config_file: Option<&brassclaw_reborn_config::RebornConfigFile>,
) -> anyhow::Result<TurnRunnerSettings> {
    let mut settings = TurnRunnerSettings::default();
    if let Some(runner) = config_file.and_then(|file| file.runner.as_ref()) {
        if let Some(secs) = runner.heartbeat_interval_secs {
            if secs == 0 {
                anyhow::bail!(
                    "config file [runner].heartbeat_interval_secs must be greater than 0"
                );
            }
            settings.heartbeat_interval = Duration::from_secs(secs);
        }
        if let Some(ms) = runner.poll_interval_ms {
            if ms == 0 {
                anyhow::bail!("config file [runner].poll_interval_ms must be greater than 0");
            }
            settings.poll_interval = Duration::from_millis(ms);
        }
    }
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use brassclaw_reborn_composition::RebornCompositionProfile;

    use brassclaw_reborn_composition::{LocalTriggerAccessRole, LocalTriggerAccessSource};
    use brassclaw_reborn_config::RebornBootConfig;

    use super::test_env::{EnvGuard, lock_trigger_env};

    use super::with_run_local_trigger_fire_access_checker;
    use super::{
        RUNTIME_PROFILE_ENV, RuntimeInputCaller, RuntimeInputOptions, block_on_cli,
        build_runtime_input, build_runtime_input_with_options, resolve_google_oauth_config,
        runtime_profile_from_env,
    };

    fn clear_trigger_poller_env() -> (EnvGuard, EnvGuard) {
        (
            EnvGuard::clear("BRASSCLAW_TRIGGER_POLLER_ENABLED"),
            EnvGuard::clear("BRASSCLAW_TRIGGER_POLLER_INTERVAL_SECS"),
        )
    }

    #[tokio::test]
    async fn block_on_cli_can_run_inside_existing_tokio_runtime() {
        let value = block_on_cli(async { Ok::<_, anyhow::Error>(42) }).expect("block future");

        assert_eq!(value, 42);
    }

    #[test]
    fn build_runtime_input_maps_configured_cli_identity() {
        let _lock = lock_trigger_env();
        let (_enabled, _interval) = clear_trigger_poller_env();

        let temp = tempfile::tempdir().expect("tempdir");
        let reborn_home = temp.path().join("reborn-home");
        std::fs::create_dir_all(&reborn_home).expect("mkdir");
        std::fs::write(
            reborn_home.join("config.toml"),
            r#"
[identity]
tenant = "custom-tenant"
default_agent = "custom-agent"
default_owner = "custom-owner"
"#,
        )
        .expect("write config");
        let config = RebornBootConfig::resolve_from_env_parts(
            Some(reborn_home.into_os_string()),
            None,
            None,
        )
        .expect("boot config");

        let runtime_input =
            build_runtime_input(&config, RuntimeInputCaller::Run).expect("runtime input");

        assert_eq!(runtime_input.identity.tenant_id, "custom-tenant");
        assert_eq!(runtime_input.identity.agent_id, "custom-agent");
        assert_eq!(runtime_input.identity.source_binding_id, "reborn-cli");
        assert_eq!(runtime_input.identity.reply_target_binding_id, "reborn-cli");
    }

    #[test]
    fn build_runtime_input_maps_regex_skill_activation_config() {
        let _lock = lock_trigger_env();
        let (_enabled, _interval) = clear_trigger_poller_env();

        let temp = tempfile::tempdir().expect("tempdir");
        let reborn_home = temp.path().join("reborn-home");
        std::fs::create_dir_all(&reborn_home).expect("mkdir");
        std::fs::write(
            reborn_home.join("config.toml"),
            r#"
[skills]
regex_activation_enabled = false
"#,
        )
        .expect("write config");
        let config = RebornBootConfig::resolve_from_env_parts(
            Some(reborn_home.into_os_string()),
            None,
            None,
        )
        .expect("boot config");

        let runtime_input =
            build_runtime_input(&config, RuntimeInputCaller::Run).expect("runtime input");

        assert!(!runtime_input.regex_skill_activation_enabled);
    }

    #[test]
    fn build_runtime_input_rejects_local_dev_yolo_without_host_access_confirmation() {
        let _lock = lock_trigger_env();
        let (_enabled, _interval) = clear_trigger_poller_env();
        // Use BRASSCLAW_RUNTIME_PROFILE env var to request yolo mode.
        let _profile = EnvGuard::set(RUNTIME_PROFILE_ENV, "local_yolo");

        let temp = tempfile::tempdir().expect("tempdir");
        let reborn_home = temp.path().join("reborn-home");
        std::fs::create_dir_all(&reborn_home).expect("mkdir");
        let config = RebornBootConfig::resolve_from_env_parts(
            Some(reborn_home.into_os_string()),
            None,
            None,
        )
        .expect("boot config");

        let error = match build_runtime_input(&config, RuntimeInputCaller::Run) {
            Ok(_) => panic!("local-dev-yolo requires confirmation"),
            Err(error) => error,
        };

        assert!(format!("{error:#}").contains("requires explicit disclosure acknowledgement"));
    }

    #[test]
    fn build_runtime_input_accepts_confirmed_local_dev_yolo_profile() {
        let _lock = lock_trigger_env();
        let (_enabled, _interval) = clear_trigger_poller_env();
        // Use BRASSCLAW_RUNTIME_PROFILE env var to request yolo mode.
        let _profile = EnvGuard::set(RUNTIME_PROFILE_ENV, "local_yolo");

        let temp = tempfile::tempdir().expect("tempdir");
        let reborn_home = temp.path().join("reborn-home");
        std::fs::create_dir_all(&reborn_home).expect("mkdir");
        let config = RebornBootConfig::resolve_from_env_parts(
            Some(reborn_home.into_os_string()),
            None,
            None,
        )
        .expect("boot config");

        let runtime_input = build_runtime_input_with_options(
            &config,
            RuntimeInputCaller::Run,
            RuntimeInputOptions {
                confirm_host_access: true,
            },
        )
        .expect("runtime input");
        assert!(runtime_input.grants_trusted_laptop_access());
        let services = runtime_input.services.expect("services input");
        let policy = services.runtime_policy().expect("runtime policy");

        assert_eq!(services.profile(), RebornCompositionProfile::LocalDevYolo);
        assert_eq!(
            policy.filesystem_backend.as_str(),
            "host_workspace_and_home"
        );
        assert_eq!(policy.secret_mode.as_str(), "inherited_env");
    }

    // Regression for the review point that `serve` rejected legitimate
    // `[identity].default_project` configs at runtime-input build time
    // because the unsupported-section check was shared with `run` / `repl`.
    // `serve` consumes the value, `run` does not — the discriminator
    // ensures both branches do the right thing.
    #[test]
    fn build_runtime_input_for_run_rejects_default_project() {
        let _lock = lock_trigger_env();
        let (_enabled, _interval) = clear_trigger_poller_env();

        let temp = tempfile::tempdir().expect("tempdir");
        let reborn_home = temp.path().join("reborn-home");
        std::fs::create_dir_all(&reborn_home).expect("mkdir");
        std::fs::write(
            reborn_home.join("config.toml"),
            r#"
[identity]
default_project = "project-alpha"
"#,
        )
        .expect("write config");
        let config = RebornBootConfig::resolve_from_env_parts(
            Some(reborn_home.into_os_string()),
            None,
            None,
        )
        .expect("boot config");

        let err = build_runtime_input(&config, RuntimeInputCaller::Run)
            .err()
            .expect("run must reject default_project");
        assert!(
            err.to_string().contains("default_project"),
            "error must mention the rejected field, got: {err}",
        );
    }

    #[test]
    fn build_runtime_input_for_run_rejects_default_project_when_trigger_poller_enabled() {
        let _lock = lock_trigger_env();
        let (_enabled, _interval) = clear_trigger_poller_env();

        let temp = tempfile::tempdir().expect("tempdir");
        let reborn_home = temp.path().join("reborn-home");
        std::fs::create_dir_all(&reborn_home).expect("mkdir");
        std::fs::write(
            reborn_home.join("config.toml"),
            r#"
[identity]
default_project = "project-alpha"

[trigger_poller]
enabled = true
"#,
        )
        .expect("write config");
        let config = RebornBootConfig::resolve_from_env_parts(
            Some(reborn_home.into_os_string()),
            None,
            None,
        )
        .expect("boot config");

        let err = build_runtime_input(&config, RuntimeInputCaller::Run)
            .err()
            .expect("run must reject default_project even when trigger poller is enabled");
        assert!(
            err.to_string().contains("default_project"),
            "error must mention the rejected field, got: {err}",
        );
    }

    #[allow(clippy::await_holding_lock, reason = "serializes env guards")]
    #[tokio::test]
    async fn run_trigger_poller_bootstrap_seeds_local_access_checker() {
        let _lock = lock_trigger_env();
        let (_enabled, _interval) = clear_trigger_poller_env();

        let temp = tempfile::tempdir().expect("tempdir");
        let reborn_home = temp.path().join("reborn-home");
        std::fs::create_dir_all(&reborn_home).expect("mkdir");
        std::fs::write(
            reborn_home.join("config.toml"),
            r#"
[identity]
tenant = "run-trigger-tenant"
default_owner = "run-trigger-user"
default_agent = "run-trigger-agent"

[trigger_poller]
enabled = true
"#,
        )
        .expect("write config");
        let config = RebornBootConfig::resolve_from_env_parts(
            Some(reborn_home.into_os_string()),
            None,
            None,
        )
        .expect("boot config");
        let runtime_input =
            build_runtime_input(&config, RuntimeInputCaller::Run).expect("runtime input");

        let tenant_id = brassclaw_reborn_composition::host_api::TenantId::new("run-trigger-tenant")
            .expect("tenant id");
        let user_id = brassclaw_reborn_composition::host_api::UserId::new("run-trigger-user")
            .expect("user id");
        let stale_user_id =
            brassclaw_reborn_composition::host_api::UserId::new("run-trigger-stale")
                .expect("stale user id");
        let agent_id = brassclaw_reborn_composition::host_api::AgentId::new("run-trigger-agent")
            .expect("agent id");
        let project_id =
            brassclaw_reborn_composition::host_api::ProjectId::new("run-trigger-project")
                .expect("project id");
        let user_store_path = config
            .home()
            .path()
            .join("local-dev")
            .join("reborn-local-dev.db");
        let access_store =
            brassclaw_reborn_composition::open_local_trigger_access_store(&user_store_path)
                .await
                .expect("open local trigger access store");
        access_store
            .seed_local_access(brassclaw_reborn_composition::LocalTriggerAccessSeed {
                tenant_id: &tenant_id,
                user_id: &stale_user_id,
                agent_id: Some(&agent_id),
                project_id: None,
                role: LocalTriggerAccessRole::Owner,
                source: LocalTriggerAccessSource::LocalDevRunBootstrap,
            })
            .await
            .expect("seed stale run trigger access");

        let runtime_input = with_run_local_trigger_fire_access_checker(runtime_input, &config)
            .await
            .expect("bootstrap run trigger fire access checker");

        let checker = runtime_input
            .trigger_fire_access_checker
            .expect("checker is wired");
        let allowed = checker
            .check_trigger_fire_access(brassclaw_reborn_composition::TriggerFireAccessCheck {
                tenant_id: tenant_id.clone(),
                creator_user_id: user_id,
                agent_id: Some(agent_id.clone()),
                project_id: None,
                trigger_id: brassclaw_reborn_composition::TriggerId::new(),
                fire_slot: chrono::Utc::now(),
            })
            .await
            .expect("check run trigger fire access");
        assert_eq!(
            allowed,
            brassclaw_reborn_composition::TriggerFireAccessDecision::Allowed
        );

        let project_scoped_decision = checker
            .check_trigger_fire_access(brassclaw_reborn_composition::TriggerFireAccessCheck {
                tenant_id: tenant_id.clone(),
                creator_user_id: brassclaw_reborn_composition::host_api::UserId::new(
                    "run-trigger-user",
                )
                .expect("user id"),
                agent_id: Some(agent_id.clone()),
                project_id: Some(project_id.clone()),
                trigger_id: brassclaw_reborn_composition::TriggerId::new(),
                fire_slot: chrono::Utc::now(),
            })
            .await
            .expect("check project-scoped run trigger fire access");
        assert_eq!(
            project_scoped_decision,
            brassclaw_reborn_composition::TriggerFireAccessDecision::Denied {
                reason: "trigger creator does not have active local access for this scope"
                    .to_string(),
            }
        );

        let stale_decision = checker
            .check_trigger_fire_access(brassclaw_reborn_composition::TriggerFireAccessCheck {
                tenant_id,
                creator_user_id: stale_user_id,
                agent_id: Some(agent_id),
                project_id: None,
                trigger_id: brassclaw_reborn_composition::TriggerId::new(),
                fire_slot: chrono::Utc::now(),
            })
            .await
            .expect("check stale run trigger fire access");
        assert_eq!(
            stale_decision,
            brassclaw_reborn_composition::TriggerFireAccessDecision::Denied {
                reason: "trigger creator does not have active local access for this scope"
                    .to_string(),
            }
        );
    }

    #[test]
    fn build_runtime_input_for_serve_accepts_default_project() {
        let _lock = lock_trigger_env();
        let (_enabled, _interval) = clear_trigger_poller_env();

        let temp = tempfile::tempdir().expect("tempdir");
        let reborn_home = temp.path().join("reborn-home");
        std::fs::create_dir_all(&reborn_home).expect("mkdir");
        std::fs::write(
            reborn_home.join("config.toml"),
            r#"
[identity]
default_project = "project-alpha"
"#,
        )
        .expect("write config");
        let config = RebornBootConfig::resolve_from_env_parts(
            Some(reborn_home.into_os_string()),
            None,
            None,
        )
        .expect("boot config");

        let _runtime_input = build_runtime_input(&config, RuntimeInputCaller::Serve)
            .expect("serve must accept default_project");
    }

    #[test]
    fn build_runtime_input_maps_trigger_poller_enabled_config() {
        let _lock = lock_trigger_env();
        let (_enabled, _interval) = clear_trigger_poller_env();

        let temp = tempfile::tempdir().expect("tempdir");
        let reborn_home = temp.path().join("reborn-home");
        std::fs::create_dir_all(&reborn_home).expect("mkdir");
        std::fs::write(
            reborn_home.join("config.toml"),
            r#"
[trigger_poller]
enabled = true
poll_interval_secs = 42
"#,
        )
        .expect("write config");
        let config = RebornBootConfig::resolve_from_env_parts(
            Some(reborn_home.into_os_string()),
            None,
            None,
        )
        .expect("boot config");

        let input = build_runtime_input(&config, RuntimeInputCaller::Run).expect("runtime input");

        assert!(
            input.trigger_poller.enabled,
            "[trigger_poller] enabled=true in config must reach runtime_input.trigger_poller.enabled"
        );
        assert_eq!(
            input.trigger_poller.worker.poll_interval,
            std::time::Duration::from_secs(42),
            "config poll_interval_secs must reach worker.poll_interval"
        );
    }

    #[test]
    fn build_runtime_input_env_enables_trigger_poller_with_no_config_section() {
        // No [trigger_poller] in config; env var enables → input.trigger_poller.enabled must be true.
        let _lock = lock_trigger_env();
        let _enabled = EnvGuard::set("BRASSCLAW_TRIGGER_POLLER_ENABLED", "true");
        let _interval = EnvGuard::clear("BRASSCLAW_TRIGGER_POLLER_INTERVAL_SECS");

        let temp = tempfile::tempdir().expect("tempdir");
        let reborn_home = temp.path().join("reborn-home");
        std::fs::create_dir_all(&reborn_home).expect("mkdir");
        // No config.toml written → no [trigger_poller] section at all.

        let config = RebornBootConfig::resolve_from_env_parts(
            Some(reborn_home.to_string_lossy().to_string().into()),
            None,
            None,
        )
        .expect("boot config");

        let input = build_runtime_input(&config, RuntimeInputCaller::Run).expect("runtime input");

        assert!(
            input.trigger_poller.enabled,
            "BRASSCLAW_TRIGGER_POLLER_ENABLED=true must reach input.trigger_poller.enabled through build_runtime_input"
        );
    }

    #[test]
    fn build_runtime_input_env_interval_overrides_config_interval() {
        // Config says interval=15s, env says interval=45s → env must win at the caller boundary.
        let _lock = lock_trigger_env();
        let _enabled = EnvGuard::clear("BRASSCLAW_TRIGGER_POLLER_ENABLED");
        let _interval = EnvGuard::set("BRASSCLAW_TRIGGER_POLLER_INTERVAL_SECS", "45");

        let temp = tempfile::tempdir().expect("tempdir");
        let reborn_home = temp.path().join("reborn-home");
        std::fs::create_dir_all(&reborn_home).expect("mkdir");
        std::fs::write(
            reborn_home.join("config.toml"),
            r#"
[trigger_poller]
enabled = true
poll_interval_secs = 15
"#,
        )
        .expect("write config");

        let config = RebornBootConfig::resolve_from_env_parts(
            Some(reborn_home.to_string_lossy().to_string().into()),
            None,
            None,
        )
        .expect("boot config");

        let input = build_runtime_input(&config, RuntimeInputCaller::Run).expect("runtime input");

        assert_eq!(
            input.trigger_poller.worker.poll_interval,
            std::time::Duration::from_secs(45),
            "env BRASSCLAW_TRIGGER_POLLER_INTERVAL_SECS=45 must override config poll_interval_secs=15 through build_runtime_input"
        );
    }

    #[test]
    fn build_runtime_input_rejects_invalid_trigger_poller_enabled_env() {
        // Invalid env value (`yes`) must error out through build_runtime_input,
        // not slip through to the runtime input. Closes the caller-level gap
        // for the error path; previous tests covered only happy/override paths.
        let _lock = lock_trigger_env();
        let _enabled = EnvGuard::set("BRASSCLAW_TRIGGER_POLLER_ENABLED", "yes");
        let _interval = EnvGuard::clear("BRASSCLAW_TRIGGER_POLLER_INTERVAL_SECS");

        let temp = tempfile::tempdir().expect("tempdir");
        let reborn_home = temp.path().join("reborn-home");
        std::fs::create_dir_all(&reborn_home).expect("mkdir");

        let config = RebornBootConfig::resolve_from_env_parts(
            Some(reborn_home.to_string_lossy().to_string().into()),
            None,
            None,
        )
        .expect("boot config");

        let err = match build_runtime_input(&config, RuntimeInputCaller::Run) {
            Ok(_) => panic!(
                "invalid BRASSCLAW_TRIGGER_POLLER_ENABLED must propagate as Err through build_runtime_input"
            ),
            Err(e) => e,
        };
        assert!(
            err.to_string().contains("BRASSCLAW_TRIGGER_POLLER_ENABLED"),
            "caller-level error must surface the env var name, got: {err}",
        );
    }

    #[test]
    fn resolve_google_oauth_config_returns_none_when_no_vars_set() {
        let config =
            resolve_google_oauth_config(|_| None).expect("empty env should not fail setup");

        assert!(config.is_none());
    }

    #[test]
    fn resolve_google_oauth_config_errors_when_client_id_missing() {
        let vars = HashMap::from([(
            "BRASSCLAW_REBORN_GOOGLE_OAUTH_REDIRECT_URI",
            "http://127.0.0.1:3000/api/reborn/product-auth/oauth/google/callback",
        )]);

        let error =
            resolve_google_oauth_config(|name| vars.get(name).map(|value| value.to_string()))
                .expect_err("redirect-only Google OAuth config must fail closed");

        assert!(error.to_string().contains("GOOGLE_CLIENT_ID"));
    }

    #[test]
    fn resolve_google_oauth_config_prefers_reborn_prefixed_vars() {
        let vars = HashMap::from([
            (
                "BRASSCLAW_REBORN_GOOGLE_CLIENT_ID",
                "reborn-client.apps.googleusercontent.com",
            ),
            (
                "BRASSCLAW_REBORN_GOOGLE_CLIENT_SECRET",
                "reborn-client-secret",
            ),
            (
                "BRASSCLAW_REBORN_GOOGLE_OAUTH_REDIRECT_URI",
                "http://127.0.0.1:3000/api/reborn/product-auth/oauth/google/callback",
            ),
            (
                "BRASSCLAW_REBORN_GOOGLE_HOSTED_DOMAIN_HINT",
                "reborn.example.com",
            ),
            (
                "GOOGLE_CLIENT_ID",
                "legacy-client.apps.googleusercontent.com",
            ),
            ("GOOGLE_CLIENT_SECRET", "legacy-client-secret"),
            (
                "GOOGLE_OAUTH_REDIRECT_URI",
                "http://127.0.0.1:3000/legacy/callback",
            ),
            ("GOOGLE_ALLOWED_HD", "legacy.example.com"),
        ]);

        let config =
            resolve_google_oauth_config(|name| vars.get(name).map(|value| value.to_string()))
                .expect("Google OAuth config")
                .expect("configured Google OAuth");

        assert_eq!(
            config.client.client_id.as_str(),
            "reborn-client.apps.googleusercontent.com"
        );
        assert_eq!(
            config.client.redirect_uri.as_str(),
            "http://127.0.0.1:3000/api/reborn/product-auth/oauth/google/callback"
        );
        assert!(config.client.client_secret.is_some());
        assert_eq!(
            config.hosted_domain_hint.as_deref(),
            Some("reborn.example.com")
        );
    }

    #[test]
    fn resolve_google_oauth_config_uses_legacy_client_vars_as_configuration_signal() {
        let vars = HashMap::from([
            (
                "GOOGLE_CLIENT_ID",
                "legacy-client.apps.googleusercontent.com",
            ),
            ("GOOGLE_CLIENT_SECRET", "legacy-client-secret"),
        ]);

        let error =
            resolve_google_oauth_config(|name| vars.get(name).map(|value| value.to_string()))
                .expect_err("legacy client vars without redirect URI must not be ignored");

        assert!(error.to_string().contains("GOOGLE_OAUTH_REDIRECT_URI"));
    }
}
